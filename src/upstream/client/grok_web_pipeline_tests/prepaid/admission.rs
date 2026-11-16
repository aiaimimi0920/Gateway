//! Exact admission and two overlapping requests sharing the final token reservation.
use super::*;
fn reservation(fixture: &Prepaid) -> i64 {
    // One 12-byte UTF-8 text part, using the existing admission estimate ratio.
    (3.0 * fixture.state.config.quota_pre_deduct_estimate_ratio).ceil() as i64
}
#[test]
fn grok_web_token_exact_reservation_admits_unknown_usage() {
    run(exact);
}
async fn exact() {
    for endpoint in ["chat/completions", "responses"] {
        for stream in [false, true] {
            let reply = wire::Reply::default();
            let peer = wire::Server::start(reply).await;
            let fixture = Prepaid::new(&peer).await;
            let amount = reservation(&fixture);
            assert!(amount > 0);
            fixture.token_remaining(amount).await;
            let response = fixture.request(&peer, endpoint, stream).await;
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
                assert!(value["usage"].is_null());
            }
            fixture.audit("completed").await;
            assert_eq!(fixture.tokens().await, 0);
            assert_wire(&peer);
            fixture.finish().await;
            peer.finish().await;
        }
    }
}
#[test]
fn grok_web_token_competing_requests_admit_only_one_and_cancel_refunds_once() {
    run(competing);
}
async fn competing() {
    for endpoint in ["chat/completions", "responses"] {
        let mut reply = wire::Reply::default();
        reply.frames.pop();
        let peer = wire::Server::start(reply).await;
        let fixture = Prepaid::new(&peer).await;
        let amount = reservation(&fixture);
        fixture.token_remaining(amount).await;
        // Both futures are polled concurrently; the admitted stream stays open.
        let (first, second) = tokio::join!(
            fixture.request(&peer, endpoint, true),
            fixture.request(&peer, endpoint, true)
        );
        let (accepted, rejected) = if first.status() == StatusCode::OK {
            (first, second)
        } else {
            (second, first)
        };
        assert_eq!(accepted.status(), StatusCode::OK);
        assert_eq!(rejected.status(), StatusCode::TOO_MANY_REQUESTS);
        let bytes = rejected.into_body().collect().await.unwrap().to_bytes();
        let error: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(error["error"]["code"], "token_balance_exhausted");
        let mut body = accepted.into_body();
        tokio::time::timeout(Duration::from_secs(3), async {
            let mut seen = String::new();
            while !seen.contains("合成答案") {
                if let Ok(bytes) = body.frame().await.unwrap().unwrap().into_data() {
                    seen.push_str(&String::from_utf8_lossy(&bytes));
                }
                assert!(seen.len() < 16384);
            }
            assert!(!seen.contains("[DONE]") && !seen.contains("response.completed"));
        })
        .await
        .unwrap();
        assert_eq!(
            fixture.tokens().await,
            0,
            "rejected request must not refund the admitted reservation"
        );
        assert_wire(&peer);
        drop(body);
        peer.wait_closed().await;
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
                if rows.len() == 2
                    && rows
                        .iter()
                        .all(|row| !matches!(row.status.as_str(), "started" | "running"))
                {
                    assert_eq!(rows.iter().filter(|row| row.status == "failed").count(), 1);
                    assert_eq!(
                        rows.iter().filter(|row| row.status == "cancelled").count(),
                        1
                    );
                    for row in rows {
                        assert_eq!(row.access_key_id.as_deref(), Some(fixture.id.as_str()));
                        assert_eq!(
                            (row.prompt_tokens, row.completion_tokens, row.total_tokens),
                            (None, None, None)
                        );
                    }
                    while Arc::strong_count(&fixture.state) > 1 {
                        tokio::task::yield_now().await;
                    }
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        assert_eq!(
            fixture.tokens().await,
            amount,
            "exactly one reservation must be refunded"
        );
        assert_wire(&peer);
        fixture.finish().await;
        peer.finish().await;
    }
}
