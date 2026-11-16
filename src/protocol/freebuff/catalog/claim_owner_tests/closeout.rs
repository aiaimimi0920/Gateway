//! Explicit original-claim end -> final receipt archive -> active slot release, over real SQLite/TCP.
use super::*;

pub(super) async fn count(db: &LocalRuntime, closed: bool) -> i64 {
    let sql = if closed {
        "SELECT count(*) FROM local_freebuff_session_closeouts"
    } else {
        "SELECT count(*) FROM local_freebuff_session_claims"
    };
    sqlx::query_scalar(sql).fetch_one(&db.pool).await.unwrap()
}

#[tokio::test]
async fn freebuff_claim_final_end_archives_original_seal_and_refund_before_new_preparation() {
    for refund in [0.0, 4.25] {
        let (root, db) = setup().await;
        let (session, device) = account();
        let requests = session.business_requests(&device, "open").unwrap();
        let prepared = pending(&db, &requests).await;
        let claim = prepared.claim().clone();
        drop(prepared);
        let original = row(&db).await;
        let seal = std::fs::read(root.join("access-key-seal.json")).unwrap();
        let peer = fixture::Peer::start(vec![fixture::Spec::json(
            200,
            json!({"status":"ended",
            "freebucksRefund":refund, "freebucksRefundSources":{"daily":refund,"wallet":0}}),
        )])
        .await;
        let recovery_requests = requests.recovery_requests();
        let ended = with_test_origin(
            peer.origin.clone(),
            recovery(&db, &recovery_requests)
                .await
                .end(&recovery_requests, || Ok(()), BUDGET),
        )
        .await
        .unwrap();
        assert!(ended.is_final());
        assert_eq!(ended.refund, Some(refund));
        assert_eq!((count(&db, false).await, count(&db, true).await), (0, 1));
        let stored: (String, String) = sqlx::query_as(
            "SELECT claim_sealed,receipt_sealed FROM local_freebuff_session_closeouts",
        )
        .fetch_one(&db.pool)
        .await
        .unwrap();
        assert_eq!(stored.0, original.2);
        assert!(!stored.1.contains("freebucksRefund"));
        let attempt = uuid::Uuid::parse_str(claim.attempt_id()).unwrap();
        let archived = db
            .freebuff_closeout("pool", "account", &recovery_requests, attempt)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(archived.model_key, "open");
        assert_eq!(archived.receipt.refund, Some(refund));
        assert_eq!(archived.receipt.refund_sources.unwrap().wallet, 0.0);
        {
            let hits = peer.hits.lock().unwrap();
            assert_eq!(hits.len(), 1);
            let hit = &hits[0];
            assert_eq!(hit.method, "DELETE");
            assert_eq!(hit.uri.path(), "/api/v1/freebuff/session/attempt");
            assert!(hit.body.is_empty());
            assert_eq!(hit.headers["x-freebuff-instance-id"], claim.instance_id());
            assert_eq!(
                hit.headers["x-freebuff-desktop-attempt-id"],
                claim.attempt_id()
            );
            assert!(hit.headers.get("x-freebuff-wallet-spend-limit").is_none());
            assert!(hit.headers.get("x-freebuff-model").is_none());
            use base64::Engine;
            let public = base64::engine::general_purpose::URL_SAFE_NO_PAD
                .decode(device.key.public_key())
                .unwrap();
            business_tests::verify(hit, &public, "synthetic-fetch");
        }
        db.close().await;
        let rebuilt = LocalRuntime::open(&root).await.unwrap();
        assert_eq!(
            std::fs::read(root.join("access-key-seal.json")).unwrap(),
            seal
        );
        assert_eq!(
            rebuilt
                .freebuff_closeout("pool", "account", &recovery_requests, attempt)
                .await
                .unwrap()
                .unwrap()
                .receipt
                .refund,
            Some(refund)
        );
        let next = pending(&rebuilt, &requests).await;
        assert_ne!(next.claim().attempt_id(), claim.attempt_id());
        assert_eq!(
            peer.count(),
            1,
            "release/read/rebuild never buys another session"
        );
        drop(next);
        peer.close().await;
        cleanup(&root, rebuilt).await;
    }
}

#[tokio::test]
async fn freebuff_claim_pending_grace_and_missing_refund_keep_original_until_explicit_final_end() {
    let (root, db) = setup().await;
    let (session, device) = account();
    let requests = session.business_requests(&device, "open").unwrap();
    drop(pending(&db, &requests).await);
    let original = row(&db).await;
    let scope = requests.recovery_requests();
    let claim = recovery(&db, &scope).await.claim().clone();
    let peer = fixture::Peer::start(vec![]).await;
    for body in [
        json!({"status":"ended", "freebucksRefundPending":true}),
        json!({"status":"ended", "freebucksRefund":0, "instanceId":claim.instance_id()}),
        json!({"status":"ended"}),
    ] {
        peer.push(fixture::Spec::json(200, body));
        let receipt = with_test_origin(
            peer.origin.clone(),
            recovery(&db, &scope).await.end(&scope, || Ok(()), BUDGET),
        )
        .await
        .unwrap();
        assert!(!receipt.is_final());
        assert_eq!(row(&db).await, original);
        assert_eq!(count(&db, true).await, 0);
    }
    peer.push(fixture::Spec::json(
        200,
        json!({"status":"ended", "freebucksRefund":0}),
    ));
    with_test_origin(
        peer.origin.clone(),
        recovery(&db, &scope).await.end(&scope, || Ok(()), BUDGET),
    )
    .await
    .unwrap();
    assert_eq!((count(&db, false).await, count(&db, true).await), (0, 1));
    assert_eq!(peer.count(), 4, "only four explicit original-claim DELETEs");
    peer.close().await;
    cleanup(&root, db).await;
}

#[tokio::test]
async fn freebuff_claim_end_can_reconcile_locked_original_model_but_archive_stays_authority_bound()
{
    let (root, db) = setup().await;
    let (mut session, device) = account();
    let requests = session.business_requests(&device, "open").unwrap();
    let prepared = pending(&db, &requests).await;
    let attempt = uuid::Uuid::parse_str(prepared.claim().attempt_id()).unwrap();
    drop(prepared);
    drop(requests);
    let mut body = tests::body();
    body["rows"][0]["access"] = json!("locked");
    session.catalog = Some(ModelCatalog::parse(&serde_json::to_vec(&body).unwrap()).unwrap());
    assert!(session.business_requests(&device, "open").is_err());
    let scope = session.recovery_requests(&device).unwrap();
    let peer = fixture::Peer::start(vec![fixture::Spec::json(
        200,
        json!({"status":"ended", "freebucksRefund":0}),
    )])
    .await;
    with_test_origin(
        peer.origin.clone(),
        recovery(&db, &scope).await.end(&scope, || Ok(()), BUDGET),
    )
    .await
    .unwrap();
    let (other, key) = synthetic("foreign", ClientSurface::Cli, BusinessOrigin::Codebuff);
    assert!(db
        .freebuff_closeout(
            "pool",
            "account",
            &other.recovery_requests(&key).unwrap(),
            attempt
        )
        .await
        .is_err());
    assert!(db
        .freebuff_closeout("pool", "other", &scope, attempt)
        .await
        .unwrap()
        .is_none());
    assert_eq!(
        db.freebuff_closeout("pool", "account", &scope, attempt)
            .await
            .unwrap()
            .unwrap()
            .model_key,
        "open"
    );
    peer.close().await;
    cleanup(&root, db).await;
}
