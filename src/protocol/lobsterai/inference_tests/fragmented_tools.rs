//! Fragmented function identity must survive Messages/Responses conversion without invented defaults.
use super::super::test_support::{Peer, Reply};
use super::*;
fn fragments() -> Vec<u8> {
    let mut frames = Vec::new();
    for calls in [
        json!([{"index":0,"id":"call-0"},{"index":1,"id":"call-1"}]),
        json!([{"index":1,"function":{"name":"look"}},{"index":0,"function":{"name":"look"}}]),
        json!([{"index":0,"function":{"name":"up","arguments":""}},{"index":1,"function":{"name":"up","arguments":""}}]),
        json!([{"index":0,"function":{"arguments":native::args().to_string()}},{"index":1,"function":{"arguments":native::args().to_string()}}]),
    ] {
        frames.push(format!("data: {}\n\n",json!({"choices":[{"index":0,"delta":{"tool_calls":calls,"function_call":null},"finish_reason":null}]})));
    }
    frames.push(format!(
        "data: {}\n\n",
        json!({"choices":[],"usage":{"prompt_tokens":2,"completion_tokens":3,"total_tokens":5}})
    ));
    frames.push("data: [DONE]\n\n".into());
    frames.concat().into_bytes()
}
#[tokio::test]
async fn lobsterai_fragmented_tools_json_sse_and_two_turns() {
    Box::pin(async {
        for endpoint in ENDPOINTS {
            for stream in [false, true] {
                let raw = fragments();
                let peer = Peer::start(vec![
                    Reply::chunks(raw.chunks(7).map(|s| s.to_vec()).collect())
                        .header("content-type", "text/event-stream"),
                    Reply::raw(200, native::reply("chat", false, false, true))
                        .header("content-type", "application/json"),
                ])
                .await;
                let fixture = Box::pin(Fixture::with_http(
                    peer.builder().build().unwrap(),
                    peer.origin.clone(),
                ))
                .await;
                super::super::transport::TEST_ORIGIN
                    .scope(peer.origin.clone(), async {
                        let mut req = public_wire::request_body(endpoint, stream);
                        req["model"] = MODEL.into();
                        let response = fixture.open(endpoint, req).await;
                        let status = response.status();
                        let text = drain(response.into_body()).await;
                        assert_eq!(status, StatusCode::OK, "{text}");
                        let calls = public_wire::calls(&text, endpoint, stream);
                        assert_eq!(calls.len(), 2, "{endpoint}/{stream}: {text}");
                        for (index, call) in calls.iter().enumerate() {
                            assert_eq!(call.id, format!("call-{index}"));
                            assert_eq!(call.name, "lookup");
                            assert_eq!(call.args, native::args());
                        }
                        assert_eq!(fixture.audits(1).await[0].total_tokens, Some(5));
                        let mut req = public_wire::followup(endpoint, stream, &calls);
                        req["model"] = MODEL.into();
                        let response = fixture.open(endpoint, req).await;
                        assert_eq!(response.status(), StatusCode::OK);
                        assert!(drain(response.into_body()).await.contains("合成答案"));
                        assert_eq!(
                            fixture
                                .audits(2)
                                .await
                                .iter()
                                .filter(|r| r.status == "completed")
                                .count(),
                            2
                        );
                        assert_eq!(fixture.remaining().await, 0);
                    })
                    .await;
                peer.inspect(|s| assert_eq!(s.len(), 2));
                fixture.finish().await;
            }
        }
    })
    .await;
}
