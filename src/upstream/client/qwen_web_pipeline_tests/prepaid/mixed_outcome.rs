//! Cancellation returns only its own reservation while another admitted request settles usage.
use super::*;
use concurrent_usage::{consume, wait_balance};

#[test]
fn qwen_web_token_mixed_success_cancel_preserves_usage_in_both_orders() {
    run(mixed);
}
async fn mixed() {
    for endpoint in ["chat/completions", "responses"] {
        for cancel_first in [false, true] {
            let mut complete = wire::Reply::default();
            complete.chunks[1] = bytes::Bytes::from_static(
                b"data: {\"done\":true,\"usage\":{\"input_tokens\":2,\"output_tokens\":9999}}\n\n",
            );
            let success_peer = wire::Server::start(complete).await;
            let mut held = wire::Reply::default();
            held.chunks.pop();
            held.hold = true;
            let cancel_peer = wire::Server::start(held).await;
            // Separate task-scoped loopback targets need a direct test client, not one peer's proxy.
            // The cfg(test) transport still refuses to dispatch without an explicit loopback scope.
            let client = rquest::Client::builder()
                .emulation(rquest_util::Emulation::Chrome136)
                .no_proxy()
                .timeout(Duration::from_secs(5))
                .retry(rquest::retry::Policy::never())
                .build()
                .unwrap();
            let fixture = Prepaid::with_client(client).await;
            fixture.token_mode().await;
            let estimate =
                (3.0 * fixture.state.config.quota_pre_deduct_estimate_ratio).ceil() as i64;
            assert!(estimate > 0 && estimate * 2 < 10000);
            let (success, cancelled) = tokio::join!(
                fixture.request(&success_peer, endpoint, true),
                fixture.request(&cancel_peer, endpoint, true)
            );
            assert_eq!(success.status(), StatusCode::OK);
            assert_eq!(cancelled.status(), StatusCode::OK);
            assert_eq!(fixture.tokens().await, 10000 - 2 * estimate);
            let mut body = cancelled.into_body();
            tokio::time::timeout(Duration::from_secs(3), async {
                let mut text = String::new();
                while !text.contains("合成答案") {
                    if let Ok(bytes) = body.frame().await.unwrap().unwrap().into_data() {
                        text.push_str(&String::from_utf8_lossy(&bytes));
                    }
                    assert!(text.len() < 16384);
                }
                assert!(!text.contains("[DONE]") && !text.contains("response.completed"));
            })
            .await
            .unwrap();
            if cancel_first {
                drop(body);
                cancel_peer.wait_closed().await;
                wait_balance(&fixture, 10000 - estimate).await;
                consume(success, endpoint).await;
            } else {
                consume(success, endpoint).await;
                wait_balance(&fixture, -1 - estimate).await;
                drop(body);
                cancel_peer.wait_closed().await;
            }
            let rows = overage::settled(&fixture, 2).await;
            assert_eq!(fixture.tokens().await, -1);
            assert_eq!(
                rows.iter().filter(|row| row.status == "completed").count(),
                1
            );
            assert_eq!(
                rows.iter().filter(|row| row.status == "cancelled").count(),
                1
            );
            for row in rows {
                assert_eq!(row.resolved_model.as_deref(), Some("qwen3.7-plus"));
                assert_eq!(
                    row.route_trace.as_ref().unwrap()["realCredentialRef"],
                    "exact"
                );
                let expected = if row.status == "completed" {
                    (Some(2), Some(9999), Some(10001))
                } else {
                    (None, None, None)
                };
                assert_eq!(
                    (row.prompt_tokens, row.completion_tokens, row.total_tokens),
                    expected
                );
            }
            let response = fixture.request(&success_peer, endpoint, true).await;
            assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
            let bytes = response.into_body().collect().await.unwrap().to_bytes();
            let error: Value = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(error["error"]["code"], "token_balance_exhausted");
            let rows = overage::settled(&fixture, 3).await;
            assert_eq!(rows.iter().filter(|row| row.status == "failed").count(), 1);
            assert_eq!(fixture.tokens().await, -1);
            assert_wire(&success_peer);
            assert_wire(&cancel_peer);
            fixture.finish().await;
            success_peer.finish().await;
            cancel_peer.finish().await;
        }
    }
}
