//! Actual Content-Type and terminal signals control success, refunds and unknown usage.
use super::super::test_support::{Peer, Reply};
use super::*;
#[tokio::test]
async fn lobsterai_response_type_and_terminal_variants_cross_public_protocols() {
    Box::pin(async {
        let full = String::from_utf8(native::reply("chat", false, true, true)).unwrap();
        let only_finish = full.replace("data: [DONE]\n\n", "");
        let only_done = full.replace("\"finish_reason\":\"stop\"", "\"finish_reason\":null");
        for endpoint in ENDPOINTS {
            for (raw, content_type) in [
                (
                    native::reply("chat", false, false, true),
                    "application/json",
                ),
                (full.as_bytes().to_vec(), "text/event-stream"),
                (only_finish.as_bytes().to_vec(), "text/event-stream"),
                (only_done.as_bytes().to_vec(), "text/event-stream"),
            ] {
                for stream in [false, true] {
                    let peer = Peer::start(vec![
                        Reply::raw(200, raw.clone()).header("content-type", content_type)
                    ])
                    .await;
                    let fixture = Box::pin(Fixture::with_http(
                        peer.builder()
                            .retry(rquest::retry::Policy::never())
                            .build()
                            .unwrap(),
                        peer.origin.clone(),
                    ))
                    .await;
                    super::super::transport::TEST_ORIGIN
                        .scope(peer.origin.clone(), async {
                            let response = fixture.open(endpoint, body(endpoint, stream)).await;
                            let status = response.status();
                            let text = drain(response.into_body()).await;
                            assert_eq!(
                                status,
                                StatusCode::OK,
                                "{endpoint}/{stream}/{content_type}: {text}"
                            );
                            assert!(text.contains("合成答案"), "{text}");
                            assert_eq!(fixture.audits(1).await[0].status, "completed");
                            assert_eq!(fixture.remaining().await, 1);
                        })
                        .await;
                    peer.inspect(|seen| assert_eq!(seen.len(), 1));
                    fixture.finish().await;
                }
            }
        }
    })
    .await;
}
#[tokio::test]
async fn lobsterai_truncated_stream_and_error_tail_refund_without_success_or_replay() {
    Box::pin(async {
        let full = String::from_utf8(native::reply("chat", false, true, true)).unwrap();
        let partial = String::from_utf8(native::reply("chat", false, true, false)).unwrap();
        for endpoint in ENDPOINTS {
            for raw in [partial.clone(), format!("{full}event: error\ndata: {{\"message\":\"{KEY}\"}}\n\n"),
                "data: {\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":0}]}}]}\n\ndata: [DONE]\n\n".into()] {
                for stream in [false,true] {
                    let peer = Peer::start(vec![Reply::raw(200,raw.as_bytes().to_vec()).header("content-type","text/event-stream")]).await;
                    let fixture = Box::pin(Fixture::with_http(peer.builder().build().unwrap(),peer.origin.clone())).await;
                    super::super::transport::TEST_ORIGIN.scope(peer.origin.clone(), async {
                        let response = fixture.open(endpoint,body(endpoint,stream)).await;
                        let mut body = response.into_body(); let mut bytes = Vec::new();
                        while let Some(Ok(frame)) = body.frame().await { if let Ok(data) = frame.into_data() { bytes.extend_from_slice(&data); } }
                        drop(body);
                        let text = String::from_utf8(bytes).unwrap();
                        assert!(!text.contains(KEY) && !text.contains("response.completed") && !text.contains("[DONE]") && !text.contains("message_stop"), "{text}");
                        assert_ne!(fixture.audits(1).await[0].status,"completed");
                        assert_eq!(fixture.remaining().await,2);
                    }).await;
                    peer.inspect(|s| assert_eq!(s.len(),1));
                    fixture.finish().await;
                }
            }
        }
    }).await;
}
#[tokio::test]
async fn lobsterai_missing_usage_stays_unknown() {
    let mut value: Value =
        serde_json::from_slice(&native::reply("chat", false, false, true)).unwrap();
    value.as_object_mut().unwrap().remove("usage");
    let peer = Peer::start(vec![
        Reply::json(value).header("content-type", "application/json")
    ])
    .await;
    let fixture = Box::pin(Fixture::with_http(
        peer.builder().build().unwrap(),
        peer.origin.clone(),
    ))
    .await;
    super::super::transport::TEST_ORIGIN
        .scope(peer.origin.clone(), async {
            let response = fixture
                .open("chat/completions", body("chat/completions", false))
                .await;
            assert_eq!(response.status(), StatusCode::OK);
            let text = drain(response.into_body()).await;
            let row = &fixture.audits(1).await[0];
            assert_eq!(
                (row.prompt_tokens, row.completion_tokens, row.total_tokens),
                (None, None, None),
                "{text}"
            );
        })
        .await;
    fixture.finish().await;
}
