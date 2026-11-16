//! Observations of real Local state and a complete public purchase/end/new-purchase TCP contract.
use super::*;
pub(super) const CLAIM: &str = "/v1/internal/gateway/console/freebuff-session-claim";
pub(super) async fn counts(host: &Host) -> (i64, i64) {
    let pool = &host.state.local_runtime.as_ref().unwrap().pool;
    let active = sqlx::query_scalar("SELECT count(*) FROM local_freebuff_session_claims")
        .fetch_one(pool)
        .await
        .unwrap();
    let history = sqlx::query_scalar("SELECT count(*) FROM local_freebuff_session_closeouts")
        .fetch_one(pool)
        .await
        .unwrap();
    (active, history)
}
pub(super) fn pending(host: &Host) -> Value {
    let snapshot = host.state.route_config.snapshot();
    let context = crate::protocol::freebuff::catalog::PurchaseContext::new(
        "pool",
        "account",
        snapshot.revision().id(),
    )
    .unwrap();
    serde_json::to_value(host.state.lifecycle.freebuff_purchases.list(&context)).unwrap()
}
pub(super) async fn wait_pending<T>(host: &Host, task: &tokio::task::JoinHandle<T>) -> Value {
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            if let Some(row) = pending(host).as_array().unwrap().first() {
                assert_eq!(counts(host).await.0, 0);
                assert_eq!(
                    host.state
                        .concurrency_registry
                        .get_or_create("pool")
                        .snapshot()
                        .active_count,
                    0
                );
                return row.clone();
            }
            assert!(
                !task.is_finished(),
                "Public request ended before pending purchase"
            );
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap()
}
pub(super) async fn audits(host: &Host, expected: &[(&str, u64)], balance: i64) {
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let rows = host
                .state
                .local_runtime
                .as_ref()
                .unwrap()
                .list_audits(&crate::db::RequestAuditFilters::default())
                .await
                .unwrap();
            if rows.len() == expected.len() && rows.iter().all(|r| r.status != "running") {
                let mut actual: Vec<_> = rows
                    .iter()
                    .map(|row| {
                        assert_eq!(row.access_key_id.as_deref(), Some(host.key_id.as_str()));
                        (
                            row.status.as_str(),
                            row.route_trace.as_ref().unwrap()["routeAttemptCount"]
                                .as_u64()
                                .unwrap(),
                        )
                    })
                    .collect();
                let mut expected = expected.to_vec();
                actual.sort_unstable();
                expected.sort_unstable();
                assert_eq!(actual, expected);
                assert_eq!(host.balance().await, balance);
                assert_eq!(
                    host.state
                        .concurrency_registry
                        .get_or_create("pool")
                        .snapshot()
                        .active_count,
                    0
                );
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
}
pub(super) fn attempt(peer: &peer::Peer, index: usize) -> String {
    peer.hits
        .lock()
        .unwrap()
        .iter()
        .filter(|(_, path, _, _)| path == "/api/v1/freebuff/session/admission")
        .nth(index)
        .unwrap()
        .2["x-freebuff-desktop-attempt-id"]
        .to_str()
        .unwrap()
        .into()
}
pub(super) fn wire(peer: &peer::Peer, purchases: usize, ends: usize) {
    assert_eq!(peer.calls("/api/v1/freebuff/session/admission"), purchases);
    assert_eq!(peer.calls("/api/v1/chat/completions"), purchases);
    assert_eq!(peer.calls("/api/v1/freebuff/session/attempt"), ends);
    let finishes = peer.finishes();
    assert_eq!(finishes.len(), purchases);
    for finish in finishes {
        assert_eq!(finish["status"], "completed");
        assert_eq!(finish["totalSteps"], 1);
        assert_eq!(finish["totalCredits"], 13);
    }
}
async fn end(host: &Host, grant: &str, id: &str, total: usize) {
    let (status, journal) = host.management(CLAIM, host.input(), Some(grant)).await;
    assert_eq!(status, 200, "{journal}");
    assert_eq!(journal["claim"]["attemptId"], id);
    let mut input = host.input();
    input["attemptId"] = json!(id);
    let (status, value) = host
        .management(&format!("{CLAIM}/end"), input, Some(grant))
        .await;
    assert_eq!(status, 200, "{value}");
    assert_eq!(value["localReleased"], true);
    assert_eq!(value["receipt"]["refund"], 0.0);
    assert_eq!(value["fromHistory"], false);
    let (status, journal) = host.management(CLAIM, host.input(), Some(grant)).await;
    assert_eq!(status, 200, "{journal}");
    assert!(journal["claim"].is_null());
    let history = journal["history"].as_array().unwrap();
    assert_eq!(history.len(), total);
    assert!(history
        .iter()
        .any(|r| r["attemptId"] == id && r["receipt"]["refund"] == 0.0));
}
#[tokio::test]
async fn freebuff_public_chat_requires_explicit_original_end_before_a_new_purchase() {
    let peer = peer::Peer::new().await;
    let host = Box::pin(Host::new(&peer)).await;
    host.seed_device(&peer).await;
    let grant = host.grant().await;
    let original = serde_json::to_value(host.state.route_config.snapshot().document()).unwrap();
    let revision = host
        .state
        .route_config
        .snapshot()
        .revision()
        .id()
        .to_owned();
    let mut ids = Vec::new();
    let mut expected = Vec::new();
    for index in 0..2 {
        let task = host.public(request());
        let view = host.pending(&grant, &task).await;
        assert_eq!(host.decision(&grant, &view, "approve").await, 200);
        assert_eq!(task.await.unwrap(), (200, peer::completion()));
        ids.push(attempt(&peer, index));
        expected.push(("completed", 1));
        audits(&host, &expected, 9 - index as i64).await;
        assert_eq!(counts(&host).await, (1, index as i64));
        wire(&peer, index + 1, index);
        let (status, body) = host.public(request()).await.unwrap();
        assert_eq!(status, 409, "{body}");
        assert_eq!(body["error"]["code"], "freebuff_public_existing_claim");
        expected.push(("failed", 0));
        audits(&host, &expected, 9 - index as i64).await;
        assert!(pending(&host).as_array().unwrap().is_empty());
        wire(&peer, index + 1, index);
        end(&host, &grant, &ids[index], index + 1).await;
        assert_eq!(counts(&host).await, (0, index as i64 + 1));
        wire(&peer, index + 1, index + 1);
    }
    assert_ne!(ids[0], ids[1]);
    assert_eq!(
        serde_json::to_value(host.state.route_config.snapshot().document()).unwrap(),
        original
    );
    assert_eq!(host.state.route_config.snapshot().revision().id(), revision);
    host.close().await;
    peer.server.close().await;
}
