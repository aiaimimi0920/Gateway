//! The composed purchase owner, real sealed SQLite and signed loopback metadata; no public sender.
use super::*;
use crate::protocol::freebuff::catalog::PurchaseContext;
use std::cell::Cell;

fn context() -> PurchaseContext {
    PurchaseContext::new("pool", "account", "pinned-revision").unwrap()
}
fn priced() -> Value {
    json!({"status":"none", "freebucks":{"balance":20,"daily":{"remaining":8},
        "wallet":{"balance":12},"prices":{"open":10},"soloDiscount":0,
        "claimableGrantFreebucks":0,"quotaExempt":false,"spendWithoutAsking":false}})
}
async fn count(db: &LocalRuntime) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM local_freebuff_session_claims")
        .fetch_one(&db.pool)
        .await
        .unwrap()
}
fn code(
    result: Result<
        crate::local_runtime::freebuff_sessions::ActiveAdmission,
        crate::error::GatewayError,
    >,
) -> String {
    result
        .err()
        .expect("must not acquire a session")
        .code
        .unwrap()
}

#[tokio::test]
async fn freebuff_purchase_requires_one_current_quote_not_a_confirmation_boolean() {
    let (root, db) = setup().await;
    let (session, device) = account();
    let requests = session.business_requests(&device, "open").unwrap();
    let peer = fixture::Peer::start(vec![]).await;
    with_test_origin(peer.origin.clone(), async {
        assert_eq!(
            code(
                db.admit_freebuff_for_request(&context(), &requests, None, || Ok(()), BUDGET)
                    .await
            ),
            "freebuff_purchase_quote_required"
        );
        assert_eq!(peer.count(), 0);
        assert_eq!(count(&db).await, 0);
    })
    .await;
    peer.close().await;
    cleanup(&root, db).await;
}

#[tokio::test]
async fn freebuff_purchase_rechecks_all_frozen_amounts_and_flags_before_creating_claim() {
    let (root, db) = setup().await;
    let (session, device) = account();
    let requests = session.business_requests(&device, "open").unwrap();
    let peer = fixture::Peer::start(vec![]).await;
    with_test_origin(peer.origin.clone(), async {
        for pointer in [
            "/freebucks/balance",
            "/freebucks/daily/remaining",
            "/freebucks/wallet/balance",
            "/freebucks/prices/open",
            "/freebucks/soloDiscount",
            "/freebucks/claimableGrantFreebucks",
            "/freebucks/quotaExempt",
            "/freebucks/spendWithoutAsking",
        ] {
            peer.push(fixture::Spec::json(200, priced()));
            let quote = requests.purchase_quote(&context(), BUDGET).await.unwrap();
            let displayed = serde_json::to_value(quote.view()).unwrap();
            assert_eq!(displayed["meter"]["sessionPrice"], 10.0);
            assert_eq!(displayed["walletSpend"], 2.0);
            let mut changed = priced();
            *changed.pointer_mut(pointer).unwrap() =
                if pointer.ends_with("Exempt") || pointer.ends_with("Asking") {
                    json!(true)
                } else {
                    json!(11)
                };
            peer.push(fixture::Spec::json(200, changed));
            assert_eq!(
                code(
                    db.admit_freebuff_for_request(
                        &context(),
                        &requests,
                        Some(quote.confirm()),
                        || Ok(()),
                        BUDGET
                    )
                    .await
                ),
                "freebuff_purchase_quote_changed",
                "{pointer}"
            );
            assert_eq!(count(&db).await, 0);
            assert_eq!(
                displayed["meter"]["sessionPrice"], 10.0,
                "the shown quote stays frozen"
            );
        }
        assert!(peer.hits.lock().unwrap().iter().all(|h| h.method == "GET"));
    })
    .await;
    peer.close().await;
    cleanup(&root, db).await;
}

#[tokio::test]
async fn freebuff_purchase_quote_cannot_move_between_exact_account_revision_model_or_authority() {
    let (root, db) = setup().await;
    let (session, device) = account();
    let requests = session.business_requests(&device, "open").unwrap();
    let peer = fixture::Peer::start(vec![]).await;
    with_test_origin(peer.origin.clone(), async {
        for next in [
            PurchaseContext::new("other", "account", "pinned-revision").unwrap(),
            PurchaseContext::new("pool", "other", "pinned-revision").unwrap(),
            PurchaseContext::new("pool", "account", "new-revision").unwrap(),
        ] {
            peer.push(fixture::Spec::json(200, priced()));
            let quote = requests
                .purchase_quote(&context(), BUDGET)
                .await
                .unwrap()
                .confirm();
            let hits = peer.count();
            assert_eq!(
                code(
                    db.admit_freebuff_for_request(&next, &requests, Some(quote), || Ok(()), BUDGET)
                        .await
                ),
                "freebuff_purchase_quote_mismatch"
            );
            assert_eq!(
                peer.count(),
                hits,
                "mismatch is rejected before another signed GET"
            );
        }
        let (foreign, key) = synthetic(
            "rotated-token",
            ClientSurface::Cli,
            BusinessOrigin::Codebuff,
        );
        let foreign = foreign.business_requests(&key, "open").unwrap();
        peer.push(fixture::Spec::json(200, priced()));
        let quote = requests
            .purchase_quote(&context(), BUDGET)
            .await
            .unwrap()
            .confirm();
        let hits = peer.count();
        assert_eq!(
            code(
                db.admit_freebuff_for_request(&context(), &foreign, Some(quote), || Ok(()), BUDGET)
                    .await
            ),
            "freebuff_purchase_quote_mismatch"
        );
        assert_eq!(peer.count(), hits);
        assert_eq!(count(&db).await, 0);
    })
    .await;
    peer.close().await;
    cleanup(&root, db).await;
}

#[tokio::test]
async fn freebuff_purchase_fence_is_rechecked_after_metadata_and_after_commit() {
    let (root, db) = setup().await;
    let (session, device) = account();
    let requests = session.business_requests(&device, "open").unwrap();
    let peer = fixture::Peer::start(vec![
        fixture::Spec::json(200, priced()),
        fixture::Spec::json(200, priced()),
    ])
    .await;
    with_test_origin(peer.origin.clone(), async {
        let quote = requests.purchase_quote(&context(), BUDGET).await.unwrap().confirm();
        assert_eq!(code(db.admit_freebuff_for_request(&context(), &requests, Some(quote), || {
            if peer.count() >= 2 { Err(crate::error::GatewayError::conflict("revoked").with_code("synthetic_fence")) } else { Ok(()) }
        }, BUDGET).await), "synthetic_fence");
        assert_eq!(count(&db).await, 0);
        peer.push(fixture::Spec::json(200, priced())); peer.push(fixture::Spec::json(200, priced()));
        let quote = requests.purchase_quote(&context(), BUDGET).await.unwrap().confirm();
        let reached = Arc::new(tokio::sync::Notify::new()); let resume = Arc::new(tokio::sync::Notify::new());
        let allowed = Cell::new(true);
        crate::local_runtime::freebuff_sessions::TEST_PREPARE_CHECKPOINT.scope((true, reached.clone(), resume.clone()), async {
            let input = context();
            let operation = db.admit_freebuff_for_request(&input, &requests, Some(quote), || {
                if allowed.get() { Ok(()) } else { Err(crate::error::GatewayError::conflict("revoked").with_code("synthetic_fence")) }
            }, BUDGET);
            tokio::pin!(operation);
            tokio::select! { _ = reached.notified() => {}, _ = &mut operation => panic!("must stop after durable commit") }
            assert_eq!(count(&db).await, 1);
            allowed.set(false); resume.notify_one();
            assert_eq!(code(operation.await), "synthetic_fence");
        }).await;
        assert_eq!(count(&db).await, 1, "no unconfirmed obligation is erased");
        assert!(peer.hits.lock().unwrap().iter().all(|h| h.method == "GET"));
    }).await;
    peer.close().await;
    cleanup(&root, db).await;
}

#[tokio::test]
async fn freebuff_purchase_commits_once_then_restores_original_claim_without_new_spend_consent() {
    let (root, db) = setup().await;
    let (session, device) = account();
    let requests = session.business_requests(&device, "open").unwrap();
    let peer = fixture::Peer::start(vec![
        fixture::Spec::json(200, priced()),
        fixture::Spec::json(200, priced()),
    ])
    .await;
    with_test_origin(peer.origin.clone(), async {
        let quote = requests.purchase_quote(&context(), BUDGET).await.unwrap().confirm();
        let reached = Arc::new(tokio::sync::Notify::new()); let resume = Arc::new(tokio::sync::Notify::new());
        let claim = crate::local_runtime::freebuff_sessions::TEST_PREPARE_CHECKPOINT.scope((true, reached.clone(), resume.clone()), async {
            let input = context();
            let operation = db.admit_freebuff_for_request(&input, &requests, Some(quote), || Ok(()), BUDGET);
            tokio::pin!(operation);
            tokio::select! { _ = reached.notified() => {}, _ = &mut operation => panic!("must persist before sending") }
            let attempt = row(&db).await.1;
            let claim = SessionClaim { scope: requests.binding.scope, instance: format!("cli:{attempt}"), attempt };
            peer.push(fixture::Spec::json(200, active(&claim, "open")));
            resume.notify_one();
            let acquired = operation.await.unwrap();
            assert_eq!(acquired.active().claim().attempt_id(), claim.attempt_id());
            drop(acquired); claim
        }).await;
        let persisted = row(&db).await;
        peer.push(fixture::Spec::json(200, active(&claim, "open")));
        let acquired = db.admit_freebuff_for_request(&context(), &requests, None, || Ok(()), BUDGET).await.unwrap();
        assert_eq!(acquired.active().claim().attempt_id(), claim.attempt_id()); drop(acquired);
        assert_eq!(row(&db).await, persisted);
        let hits = peer.hits.lock().unwrap();
        assert_eq!(hits.len(), 4);
        assert_eq!(hits.iter().filter(|h| h.method == "POST").count(), 1);
        assert_eq!(hits[3].headers["x-freebuff-instance-id"], claim.instance_id());
        assert_eq!(hits[2].headers["x-freebuff-wallet-spend-limit"], "session", "not a monetary ceiling");
    }).await;
    peer.close().await;
    cleanup(&root, db).await;
}
