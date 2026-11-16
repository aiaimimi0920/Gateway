//! Real inter-instance insertion and prepare cancellation; no second buying identity or lock owner.
use super::*;
use crate::protocol::freebuff::catalog::PurchaseContext;
use std::cell::Cell;
fn context() -> PurchaseContext {
    PurchaseContext::new("pool", "account", "revision").unwrap()
}
fn priced() -> Value {
    json!({"status":"none","freebucks":{"balance":20,"daily":{"remaining":8},"wallet":{"balance":12},"prices":{"open":10}}})
}
async fn count(db: &LocalRuntime) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM local_freebuff_session_claims")
        .fetch_one(&db.pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn freebuff_purchase_race_uses_returned_recovery_guard_not_another_lock_or_post() {
    let (root, db) = setup().await;
    let other = LocalRuntime::open(&root).await.unwrap();
    let (session, device) = account();
    let requests = session.business_requests(&device, "open").unwrap();
    let peer = fixture::Peer::start(vec![fixture::Spec::json(200, priced())]).await;
    with_test_origin(peer.origin.clone(), async {
        let quote = requests.purchase_quote(&context(), BUDGET).await.unwrap().confirm();
        let mut metadata = fixture::Spec::json(200, priced()); metadata.body_delay = Duration::from_millis(200);
        peer.push(metadata);
        let reached = Arc::new(tokio::sync::Notify::new());
        TEST_BODY_STARTED.scope(reached.clone(), async {
            let input = context();
            let operation = db.admit_freebuff_for_request(&input, &requests, Some(quote), || Ok(()), BUDGET);
            tokio::pin!(operation);
            tokio::select! { _ = reached.notified() => {}, _ = &mut operation => panic!("must be waiting for fresh metadata") }
            // Another owner can commit here: the first operation holds no account guard or SQLite tx.
            let prepared = pending(&other, &requests).await;
            let claim = prepared.claim().clone(); drop(prepared);
            let persisted = row(&other).await;
            peer.push(fixture::Spec::json(200, active(&claim, "open")));
            let acquired = operation.await.unwrap();
            assert_eq!(acquired.active().claim().attempt_id(), claim.attempt_id()); drop(acquired);
            assert_eq!(row(&db).await, persisted);
        }).await;
        let hits = peer.hits.lock().unwrap();
        assert_eq!(hits.len(), 3);
        assert!(hits.iter().all(|h| h.method == "GET"), "the racing prepared claim is only reconciled");
    }).await;
    other.close().await;
    peer.close().await;
    cleanup(&root, db).await;
}

#[tokio::test]
async fn freebuff_purchase_precommit_revocation_rolls_back_but_postcommit_cancellation_retains_claim(
) {
    let (root, db) = setup().await;
    let (session, device) = account();
    let requests = session.business_requests(&device, "open").unwrap();
    let peer = fixture::Peer::start(vec![]).await;
    with_test_origin(peer.origin.clone(), async {
        for committed in [false, true] {
            peer.push(fixture::Spec::json(200, priced())); peer.push(fixture::Spec::json(200, priced()));
            let quote = requests.purchase_quote(&context(), BUDGET).await.unwrap().confirm();
            let reached = Arc::new(tokio::sync::Notify::new()); let resume = Arc::new(tokio::sync::Notify::new());
            let allowed = Cell::new(true);
            crate::local_runtime::freebuff_sessions::TEST_PREPARE_CHECKPOINT.scope((committed, reached.clone(), resume.clone()), async {
                let input = context();
                let operation = db.admit_freebuff_for_request(&input, &requests, Some(quote), || {
                    if allowed.get() { Ok(()) } else { Err(crate::error::GatewayError::conflict("revoked").with_code("synthetic_fence")) }
                }, BUDGET);
                tokio::pin!(operation);
                tokio::select! { _ = reached.notified() => {}, _ = &mut operation => panic!("must reach durable boundary") }
                if !committed {
                    allowed.set(false); resume.notify_one();
                    assert_eq!(operation.await.err().unwrap().code.as_deref(), Some("synthetic_fence"));
                }
                // In the committed case dropping this scope cancels before any admission POST.
            }).await;
            assert_eq!(count(&db).await, i64::from(committed));
        }
        let persisted = row(&db).await;
        peer.push(fixture::Spec::json(200, json!({"status":"none"})));
        assert!(db.admit_freebuff_for_request(&context(), &requests, None, || Ok(()), BUDGET).await.is_err());
        assert_eq!(row(&db).await, persisted, "none after an unsent/unknown claim never buys a replacement");
        assert!(peer.hits.lock().unwrap().iter().all(|h| h.method == "GET"));
    }).await;
    peer.close().await;
    cleanup(&root, db).await;
}
