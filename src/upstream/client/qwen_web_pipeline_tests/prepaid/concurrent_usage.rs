//! Two admitted streams must accumulate reported usage without overwriting another reservation.
use super::*;

#[test]
fn qwen_web_token_concurrent_overage_accumulates_and_blocks_next_request() {
    run(concurrent_usage);
}
async fn concurrent_usage() {
    for endpoint in ["chat/completions", "responses"] {
        for simultaneous in [false, true] {
            let mut reply = wire::Reply::default();
            reply.chunks[1] = bytes::Bytes::from_static(
                b"data: {\"done\":true,\"usage\":{\"input_tokens\":2,\"output_tokens\":9999}}\n\n",
            );
            let peer = wire::Server::start(reply).await;
            let fixture = Prepaid::new(&peer).await;
            fixture.token_mode().await;
            let estimate =
                (3.0 * fixture.state.config.quota_pre_deduct_estimate_ratio).ceil() as i64;
            assert!(estimate > 0 && estimate * 2 < 10000);
            let (first, second) = tokio::join!(
                fixture.request(&peer, endpoint, true),
                fixture.request(&peer, endpoint, true)
            );
            assert_eq!(first.status(), StatusCode::OK);
            assert_eq!(second.status(), StatusCode::OK);
            assert_eq!(fixture.tokens().await, 10000 - 2 * estimate);
            if simultaneous {
                tokio::join!(consume(first, endpoint), consume(second, endpoint));
            } else {
                consume(first, endpoint).await;
                // The other admitted response still owns its reservation until its own terminal.
                wait_balance(&fixture, -1 - estimate).await;
                consume(second, endpoint).await;
            }
            let rows = overage::settled(&fixture, 2).await;
            assert_eq!(fixture.tokens().await, -10002);
            for row in &rows {
                assert_eq!(row.status, "completed");
                assert_eq!(
                    (row.prompt_tokens, row.completion_tokens, row.total_tokens),
                    (Some(2), Some(9999), Some(10001))
                );
                assert_eq!(row.resolved_model.as_deref(), Some("qwen3.7-plus"));
                assert_eq!(
                    row.route_trace.as_ref().unwrap()["realCredentialRef"],
                    "exact"
                );
            }
            assert_ne!(rows[0].id, rows[1].id);
            let response = fixture.request(&peer, endpoint, true).await;
            assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
            let bytes = response.into_body().collect().await.unwrap().to_bytes();
            let error: Value = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(error["error"]["code"], "token_balance_exhausted");
            let rows = overage::settled(&fixture, 3).await;
            assert_eq!(
                rows.iter().filter(|row| row.status == "completed").count(),
                2
            );
            assert_eq!(
                rows.iter()
                    .filter(|row| row.status == "failed" && row.total_tokens.is_none())
                    .count(),
                1
            );
            assert_eq!(fixture.tokens().await, -10002);
            let calls = peer.calls();
            assert_eq!(
                calls.len(),
                4,
                "only two admitted create/completion pairs may reach upstream"
            );
            assert_eq!(
                calls
                    .iter()
                    .filter(|call| call.body["model"] == "qwen3.7-plus")
                    .count(),
                2
            );
            for call in &calls {
                assert_eq!(call.headers["authorization"], "Bearer synthetic-exact");
                assert_eq!(
                    call.headers["cookie"],
                    "token=synthetic-exact; cf_clearance=synthetic-clearance"
                );
                assert_eq!(call.headers["user-agent"], "exact-UA");
            }
            fixture.finish().await;
            peer.finish().await;
        }
    }
}
pub(super) async fn consume(response: Response, endpoint: &str) {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let text = String::from_utf8_lossy(&bytes);
    assert!(text.contains("合成答案"));
    assert!(text.contains(if endpoint == "responses" {
        "response.completed"
    } else {
        "[DONE]"
    }));
}
pub(super) async fn wait_balance(fixture: &Prepaid, expected: i64) {
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            if fixture.tokens().await == expected {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
}
