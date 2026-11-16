//! Product-specific history and terminal checks reject before publication, never retry unknown work.
use super::*;
#[tokio::test]
async fn dim_public_gemini_invalid_or_incomplete_native_completion_never_succeeds() {
    Box::pin(async {
        let valid = native::reply("gemini", false, true, true);
        for raw in [
            native::reply("gemini", false, true, false),
            b"data: {bad}\n\n".to_vec(),
            b"event: error\ndata: {\"message\":\"synthetic-private-error\"}\n\n".to_vec(),
            valid[..valid.len() - 1].to_vec(),
        ] {
            let peer = peer::Peer::raw(raw).await;
            let fixture = Box::pin(Fixture::new(&peer, "gemini")).await;
            super::super::test_support::origin(peer.origin.clone(), async {
                let response = fixture
                    .open("chat/completions", body("chat/completions", true))
                    .await;
                let status = response.status();
                let text = drain(response.into_body()).await;
                assert_eq!(status, StatusCode::BAD_GATEWAY, "{text}");
                assert!(text.contains("dim_oauth_invalid_response"), "{text}");
                assert!(
                    !text.contains(ACCESS)
                        && !text.contains("synthetic-private-error")
                        && !text.contains("合成答案")
                );
                let rows = fixture.audits(1).await;
                assert_eq!(rows[0].status, "failed");
                assert_identity(&rows[0]);
                assert!(rows[0].total_tokens.is_none());
                assert_eq!(fixture.remaining().await, 2);
                assert_wire(&peer, "gemini", 1);
            })
            .await;
            fixture.finish().await;
            peer.stop().await;
        }
    })
    .await;
}
#[tokio::test]
async fn dim_public_gemini_unknown_tool_result_is_rejected_before_network_and_refunded() {
    Box::pin(async {
        for endpoint in ENDPOINTS {
            let peer = peer::Peer::new("gemini", false, false, 200).await;
            let fixture = Box::pin(Fixture::new(&peer, "gemini")).await;
            let calls = vec![public_wire::Call {
                id: "known".into(),
                name: "lookup".into(),
                args: native::args(),
            }];
            let mut request = public_wire::followup(endpoint, false, &calls);
            match endpoint {
                "messages" => {
                    request["messages"][2]["content"][0]["tool_use_id"] = json!("unknown")
                }
                "responses" => request["input"][2]["call_id"] = json!("unknown"),
                _ => request["messages"][2]["tool_call_id"] = json!("unknown"),
            };
            super::super::test_support::origin(peer.origin.clone(), async {
                let response = fixture.open(endpoint, request).await;
                let status = response.status();
                let text = drain(response.into_body()).await;
                assert_eq!(status, StatusCode::BAD_REQUEST, "{text}");
                assert!(text.contains("dim_oauth_invalid_tool_history"), "{text}");
                assert!(peer.calls.lock().unwrap().is_empty());
                assert_eq!(fixture.audits(1).await[0].status, "failed");
                assert_eq!(fixture.remaining().await, 2);
            })
            .await;
            fixture.finish().await;
            peer.stop().await;
        }
    })
    .await;
}
