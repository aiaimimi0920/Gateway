//! Additive management top-ups repay existing debt before permitting another public request.
use super::*;
use crate::access_balance::AccessBalanceStore;

#[test]
fn qwen_web_token_debt_top_up_repays_then_restores_request_admission() {
    run(recovery);
}
async fn recovery() {
    for endpoint in ["chat/completions", "responses"] {
        for stream in [false, true] {
            let mut reply = wire::Reply::default();
            reply.chunks[1] = bytes::Bytes::from_static(
                b"data: {\"done\":true,\"usage\":{\"input_tokens\":2,\"output_tokens\":9999}}\n\n",
            );
            let peer = wire::Server::start(reply).await;
            let fixture = Prepaid::new(&peer).await;
            fixture.token_mode().await;
            completed(&fixture, &peer, endpoint, stream).await;
            overage::settled(&fixture, 1).await;
            balance(&fixture, 10000, -1).await;
            rejected(&fixture, &peer, endpoint, stream).await;
            overage::settled(&fixture, 2).await;
            assert_wire(&peer);

            // Only tokenDelta is sent: neither purchased totals nor remaining debt are reset.
            fixture.top_up_tokens(1).await;
            balance(&fixture, 10001, 0).await;
            rejected(&fixture, &peer, endpoint, stream).await;
            overage::settled(&fixture, 3).await;
            balance(&fixture, 10001, 0).await;
            assert_wire(&peer);

            fixture.top_up_tokens(10001).await;
            balance(&fixture, 20002, 10001).await;
            completed(&fixture, &peer, endpoint, stream).await;
            overage::settled(&fixture, 4).await;
            balance(&fixture, 20002, 0).await;
            rejected(&fixture, &peer, endpoint, stream).await;
            let rows = overage::settled(&fixture, 5).await;
            balance(&fixture, 20002, 0).await;
            assert_eq!(
                rows.iter().filter(|row| row.status == "completed").count(),
                2
            );
            assert_eq!(rows.iter().filter(|row| row.status == "failed").count(), 3);
            for row in rows {
                if row.status == "completed" {
                    assert_eq!(
                        (row.prompt_tokens, row.completion_tokens, row.total_tokens),
                        (Some(2), Some(9999), Some(10001))
                    );
                    assert_eq!(row.resolved_model.as_deref(), Some("qwen3.7-plus"));
                    assert_eq!(
                        row.route_trace.as_ref().unwrap()["realCredentialRef"],
                        "exact"
                    );
                } else {
                    assert_eq!(
                        (row.prompt_tokens, row.completion_tokens, row.total_tokens),
                        (None, None, None)
                    );
                }
            }
            let calls = peer.calls();
            assert_eq!(
                calls.len(),
                4,
                "only the two admitted requests may create upstream work"
            );
            for call in &calls {
                assert_eq!(call.headers["authorization"], "Bearer synthetic-exact");
                assert_eq!(
                    call.headers["cookie"],
                    "token=synthetic-exact; cf_clearance=synthetic-clearance"
                );
                assert_eq!(call.headers["user-agent"], "exact-UA");
            }
            for index in [1, 3] {
                assert_eq!(calls[index].body["model"], "qwen3.7-plus");
            }
            fixture.finish().await;
            peer.finish().await;
        }
    }
}
async fn balance(fixture: &Prepaid, total: i64, remaining: i64) {
    let balance = AccessBalanceStore::from_state(&fixture.state)
        .unwrap()
        .get(&fixture.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(balance.balance_mode, "token_prepaid");
    assert_eq!(balance.total_tokens, Some(total));
    assert_eq!(balance.remaining_tokens, Some(remaining));
}
async fn completed(fixture: &Prepaid, peer: &wire::Server, endpoint: &str, stream: bool) {
    let response = fixture.request(peer, endpoint, stream).await;
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let text = String::from_utf8_lossy(&bytes);
    assert!(text.contains("合成答案"));
    if stream {
        assert!(text.contains(if endpoint == "responses" {
            "response.completed"
        } else {
            "[DONE]"
        }));
    } else {
        let value: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(value["usage"]["total_tokens"], 10001);
    }
}
async fn rejected(fixture: &Prepaid, peer: &wire::Server, endpoint: &str, stream: bool) {
    let response = fixture.request(peer, endpoint, stream).await;
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let value: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(value["error"]["code"], "token_balance_exhausted");
}
