//! Reported usage settles beyond the estimate; debt must block further upstream work.
use super::*;

#[test]
fn qwen_web_token_reported_usage_above_reservation_settles_exactly() {
    run(above_reservation);
}
async fn above_reservation() {
    check(101).await;
}
#[test]
fn qwen_web_token_reported_usage_above_balance_preserves_debt_and_blocks_next_request() {
    run(above_balance);
}
async fn above_balance() {
    check(10001).await;
}
async fn check(total: i64) {
    for endpoint in ["chat/completions", "responses"] {
        for stream in [false, true] {
            let mut reply = wire::Reply::default();
            reply.chunks[1] = bytes::Bytes::from(format!(
                "data: {{\"done\":true,\"usage\":{{\"input_tokens\":2,\"output_tokens\":{}}}}}\n\n",
                total - 2
            ));
            let peer = wire::Server::start(reply).await;
            let fixture = Prepaid::new(&peer).await;
            fixture.token_mode().await;
            let estimate =
                (3.0 * fixture.state.config.quota_pre_deduct_estimate_ratio).ceil() as i64;
            assert!(estimate > 0 && estimate < total);
            let response = fixture.request(&peer, endpoint, stream).await;
            assert_eq!(response.status(), StatusCode::OK);
            if stream {
                assert_eq!(fixture.tokens().await, 10000 - estimate);
            }
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
                assert_eq!(value["usage"]["total_tokens"], total);
            }
            let rows = settled(&fixture, 1).await;
            let row = &rows[0];
            assert_eq!(row.status, "completed");
            assert_eq!(row.resolved_model.as_deref(), Some("qwen3.7-plus"));
            assert_eq!(
                row.route_trace.as_ref().unwrap()["realCredentialRef"],
                "exact"
            );
            assert_eq!(
                (row.prompt_tokens, row.completion_tokens, row.total_tokens),
                (
                    Some(2),
                    Some(i32::try_from(total - 2).unwrap()),
                    Some(i32::try_from(total).unwrap())
                )
            );
            assert_eq!(fixture.tokens().await, 10000 - total);
            assert_wire(&peer);
            if total > 10000 {
                // Do not clamp debt to zero or allow a new upstream operation after settlement.
                let response = fixture.request(&peer, endpoint, stream).await;
                assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
                let bytes = response.into_body().collect().await.unwrap().to_bytes();
                let error: Value = serde_json::from_slice(&bytes).unwrap();
                assert_eq!(error["error"]["code"], "token_balance_exhausted");
                let rows = settled(&fixture, 2).await;
                assert_eq!(
                    rows.iter().filter(|row| row.status == "completed").count(),
                    1
                );
                assert_eq!(
                    rows.iter()
                        .filter(|row| row.status == "failed" && row.total_tokens.is_none())
                        .count(),
                    1
                );
                assert_eq!(
                    fixture.tokens().await,
                    -1,
                    "denied request must not erase or refund existing debt"
                );
                assert_wire(&peer);
            }
            fixture.finish().await;
            peer.finish().await;
        }
    }
}
pub(super) async fn settled(fixture: &Prepaid, count: usize) -> Vec<db::GatewayRequestAuditView> {
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let rows = fixture
                .state
                .local_runtime
                .as_ref()
                .unwrap()
                .list_audits(&db::RequestAuditFilters::default())
                .await
                .unwrap();
            if rows.len() == count
                && rows
                    .iter()
                    .all(|row| !matches!(row.status.as_str(), "started" | "running"))
            {
                for row in &rows {
                    assert_eq!(row.access_key_id.as_deref(), Some(fixture.id.as_str()));
                }
                while Arc::strong_count(&fixture.state) > 1 {
                    tokio::task::yield_now().await;
                }
                return rows;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap()
}
