//! 公共入口等待上游原生协议完成；错误尾部不能释放成功终态。
use super::*;
#[tokio::test]
async fn sensenova_stream_errors_and_truncation_refund_without_success_or_replay() {
    Box::pin(async {
        let error = format!(
            "event: error\ndata: {{\"type\":\"error\",\"error\":{{\"message\":\"{KEY}\"}}}}\n\n"
        );
        for endpoint in ENDPOINTS {
            let full = String::from_utf8(native::reply(family(endpoint), false, true, true)).unwrap();
            let partial = format!("{}\n\n", full.split_once("\n\n").unwrap().0);
            for raw in [
                String::new(),
                format!("{{\"error\":\"{KEY}\"}}"),
                partial.clone(),
                format!("{partial}{error}"),
                format!("{full}{error}"),
                format!("{full}data: {{"),
                "data: {\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":0}]},\"finish_reason\":\"tool_calls\"}]}\n\ndata: [DONE]\n\n".into(),
                "data: {\"error\":{\"code\":\"model_not_found\",\"message\":\"synthetic missing\"}}\n\n".into(),
            ] {
                let peer = peer::Peer::raw(raw.into_bytes()).await;
                let fixture = Box::pin(Fixture::with_fallback(&peer, true)).await;
                super::super::test_support::origin(peer.origin.clone(), async {
                    let response = fixture.open(endpoint, body(endpoint, true)).await;
                    let mut body = response.into_body();
                    let mut bytes = Vec::new();
                    tokio::time::timeout(Duration::from_secs(5), async {
                        while let Some(frame) = body.frame().await {
                            match frame {
                                Ok(f) => {
                                    if let Ok(data) = f.into_data() {
                                        bytes.extend_from_slice(&data);
                                    }
                                }
                                Err(_) => break,
                            }
                        }
                    })
                    .await
                    .unwrap();
                    drop(body);
                    let text = String::from_utf8(bytes).unwrap();
                    assert!(
                        !text.contains(KEY)
                            && !text.contains("response.completed")
                            && !text.contains("[DONE]")
                            && !text.contains("message_stop"),
                        "{endpoint}: {text}"
                    );
                    let rows = fixture.audits(1).await;
                    assert_ne!(rows[0].status, "completed", "{endpoint}: {text}");
                    assert_eq!(fixture.remaining().await, 2);
                    assert_wire(&peer, 1, endpoint);
                    peer.wait_closed().await;
                })
                .await;
                fixture.finish().await;
                peer.stop().await;
            }
        }
    })
    .await;
}
#[tokio::test]
async fn sensenova_malformed_json_body_never_replays_or_charges() {
    Box::pin(async {
        for endpoint in ENDPOINTS {
            let peer = peer::Peer::raw(b"{invalid-json".to_vec()).await;
            let fixture = Box::pin(Fixture::with_fallback(&peer, true)).await;
            super::super::test_support::origin(peer.origin.clone(), async {
                let response = fixture.open(endpoint, body(endpoint, false)).await;
                assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
                let text = drain(response.into_body()).await;
                assert!(text.contains("sensenova_response_failed"), "{text}");
                assert_ne!(fixture.audits(1).await[0].status, "completed");
                assert_eq!(fixture.remaining().await, 2);
                assert_wire(&peer, 1, endpoint);
            })
            .await;
            fixture.finish().await;
            peer.stop().await;
        }
    })
    .await;
}
