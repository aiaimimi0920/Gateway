//! Closed receipts remain sealed historical facts, even when no active secret owner remains.
use super::closeout::count;
use super::*;

async fn archive(db: &LocalRuntime, requests: &BusinessRequests<'_>) -> uuid::Uuid {
    let prepared = pending(db, requests).await;
    let attempt = uuid::Uuid::parse_str(prepared.claim().attempt_id()).unwrap();
    drop(prepared);
    let scope = requests.recovery_requests();
    let peer = fixture::Peer::start(vec![fixture::Spec::json(
        200,
        json!({"status":"ended", "freebucksRefund":0}),
    )])
    .await;
    with_test_origin(
        peer.origin.clone(),
        recovery(db, &scope).await.end(&scope, || Ok(()), BUDGET),
    )
    .await
    .unwrap();
    assert_eq!(peer.count(), 1);
    peer.close().await;
    attempt
}

#[tokio::test]
async fn freebuff_claim_closeout_only_database_never_replaces_missing_seal() {
    let (root, db) = setup().await;
    let (session, device) = account();
    let requests = session.business_requests(&device, "open").unwrap();
    let scope = requests.recovery_requests();
    let attempt = archive(&db, &requests).await;
    assert_eq!((count(&db, false).await, count(&db, true).await), (0, 1));
    let other_sealed: i64 = sqlx::query_scalar("SELECT (SELECT count(*) FROM local_access_key_secrets) + (SELECT count(*) FROM local_freebuff_device_keys) + (SELECT count(*) FROM local_freebuff_registrations) + (SELECT count(*) FROM local_freebuff_registration_replacements)")
        .fetch_one(&db.pool).await.unwrap();
    assert_eq!(other_sealed, 0, "the history must be the only sealed owner");
    let original = std::fs::read(root.join("access-key-seal.json")).unwrap();
    db.close().await;
    // Both paths are inside this test's isolated TEMP root; preserve, never regenerate the seal.
    std::fs::rename(
        root.join("access-key-seal.json"),
        root.join("seal-backup.json"),
    )
    .unwrap();
    let missing = LocalRuntime::open(&root).await.unwrap();
    assert!(!root.join("access-key-seal.json").exists());
    assert!(missing
        .freebuff_closeout("pool", "account", &scope, attempt)
        .await
        .is_err());
    assert!(missing
        .prepare_freebuff_admission(
            "pool",
            "account",
            &requests,
            SessionChargeConsent::MaySpendFreebucks
        )
        .await
        .is_err());
    assert!(!root.join("access-key-seal.json").exists());
    assert_eq!(
        (count(&missing, false).await, count(&missing, true).await),
        (0, 1)
    );
    missing.close().await;
    std::fs::rename(
        root.join("seal-backup.json"),
        root.join("access-key-seal.json"),
    )
    .unwrap();
    let rebuilt = LocalRuntime::open(&root).await.unwrap();
    assert_eq!(
        std::fs::read(root.join("access-key-seal.json")).unwrap(),
        original
    );
    assert_eq!(
        rebuilt
            .freebuff_closeout("pool", "account", &scope, attempt)
            .await
            .unwrap()
            .unwrap()
            .receipt
            .refund,
        Some(0.0)
    );
    let version: i64 = sqlx::query_scalar("PRAGMA user_version")
        .fetch_one(&rebuilt.pool)
        .await
        .unwrap();
    assert_eq!(version, 1);
    cleanup(&root, rebuilt).await;
}

#[tokio::test]
async fn freebuff_claim_closeout_corrupt_ciphertext_or_attempt_is_not_refund_or_active_capability()
{
    let (root, db) = setup().await;
    let (session, device) = account();
    let requests = session.business_requests(&device, "open").unwrap();
    let scope = requests.recovery_requests();
    let attempt = archive(&db, &requests).await;
    let original: (String, String, String) = sqlx::query_as(
        "SELECT attempt_id,claim_sealed,receipt_sealed FROM local_freebuff_session_closeouts",
    )
    .fetch_one(&db.pool)
    .await
    .unwrap();
    let foreign = uuid::Uuid::new_v4();
    for (id, claim, receipt) in [
        (attempt, "corrupt".to_string(), original.2.clone()),
        (attempt, original.1.clone(), "corrupt".to_string()),
        (attempt, original.1.clone(), "a".repeat(8193)),
        (attempt, original.1.clone(), original.1.clone()),
        (foreign, original.1.clone(), original.2.clone()),
    ] {
        sqlx::query("UPDATE local_freebuff_session_closeouts SET attempt_id=?,claim_sealed=?,receipt_sealed=?")
            .bind(id.to_string()).bind(claim).bind(receipt).execute(&db.pool).await.unwrap();
        assert!(db
            .freebuff_closeout("pool", "account", &scope, id)
            .await
            .is_err());
        assert!(db
            .recover_freebuff_admission("pool", "account", &scope)
            .await
            .unwrap()
            .is_none());
        assert_eq!((count(&db, false).await, count(&db, true).await), (0, 1));
    }
    sqlx::query(
        "UPDATE local_freebuff_session_closeouts SET attempt_id=?,claim_sealed=?,receipt_sealed=?",
    )
    .bind(original.0)
    .bind(original.1)
    .bind(original.2)
    .execute(&db.pool)
    .await
    .unwrap();
    assert_eq!(
        db.freebuff_closeout("pool", "account", &scope, attempt)
            .await
            .unwrap()
            .unwrap()
            .receipt
            .refund,
        Some(0.0)
    );
    cleanup(&root, db).await;
}
