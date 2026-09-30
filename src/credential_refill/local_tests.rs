use super::*;
use crate::local_runtime::LocalRuntime;

fn task(provider: &str, key: &str) -> CredentialRefillTaskRecord {
    serde_json::from_value(serde_json::json!({
        "id": uuid::Uuid::new_v4().to_string(), "providerId": provider, "providerLabel": provider,
        "trigger": "user_requested", "state": "pending", "requestedCount": 1,
        "targetSize": 2, "activeCredentialCount": 1, "routeRevision": "r1",
        "createdAt": now_rfc3339(), "updatedAt": now_rfc3339(), "idempotencyKey": key,
    }))
    .unwrap()
}
async fn database() -> (std::path::PathBuf, LocalRuntime) {
    let root = std::env::temp_dir().join(format!("gateway-local-refill-{}", uuid::Uuid::new_v4()));
    let db = LocalRuntime::open(&root).await.unwrap();
    (root, db)
}
async fn cleanup(root: std::path::PathBuf, db: LocalRuntime) {
    db.close().await;
    crate::local_runtime::test_support::remove_test_root(&root).await;
}

#[tokio::test]
async fn local_refill_long_lease_survives_short_task_ttl_and_delivery_lock() {
    let (root, db) = database().await;
    let original = task("a", "lease");
    local::create(&db, original.clone(), 300).await.unwrap();
    let expected = serialize_task(&original).unwrap();
    let mut claimed = original.clone();
    claimed.state = CredentialRefillTaskState::Claimed;
    claimed.claim_token = Some("owner".into());
    assert!(local::claim(&db, &expected, &claimed, 3600, 300)
        .await
        .unwrap());
    let (expires, lease): (i64, i64) =
        sqlx::query_as("SELECT expires, lease_until FROM refill_tasks WHERE id = ?")
            .bind(&claimed.id)
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert!(expires > lease);
    local::renew(&db, &claimed, "owner", 7200, 300)
        .await
        .unwrap();
    let (expires, lease): (i64, i64) =
        sqlx::query_as("SELECT expires, lease_until FROM refill_tasks WHERE id = ?")
            .bind(&claimed.id)
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert!(expires > lease);
    let guard = db.try_refill_lock(&claimed.id).await.unwrap().unwrap();
    assert!(db.try_refill_lock(&claimed.id).await.unwrap().is_none());
    assert!(local::finish(&db, &claimed, "owner", 300).await.is_err());
    // A delivery lock must never hold SQLite's writer lock across upstream I/O.
    sqlx::query("INSERT INTO provider_pricing VALUES ('unrelated', '{}')")
        .execute(&db.pool)
        .await
        .unwrap();
    drop(guard);
    cleanup(root, db).await;
}

#[tokio::test]
async fn local_refill_create_deduplicates_concurrent_instances_and_scopes_keys() {
    let (root, db) = database().await;
    let other = LocalRuntime::open(&root).await.unwrap();
    let (a, b) = tokio::join!(
        local::create(&db, task("a", "key"), 3600),
        local::create(&other, task("a", "key"), 3600)
    );
    let (a, b) = (a.unwrap(), b.unwrap());
    assert_ne!(a.created, b.created);
    assert_eq!(a.task.id, b.task.id);
    assert!(
        local::create(&db, task("b", "key"), 3600)
            .await
            .unwrap()
            .created
    );
    assert_eq!(local::pending(&db, 10).await.unwrap().len(), 2);
    other.close().await;
    cleanup(root, db).await;
}

#[tokio::test]
async fn local_refill_claim_renew_terminal_and_restart_recovery() {
    let (root, db) = database().await;
    let original = task("a", "key");
    local::create(&db, original.clone(), 3600).await.unwrap();
    let expected = serialize_task(&original).unwrap();
    let mut claimed = original.clone();
    claimed.state = CredentialRefillTaskState::Claimed;
    claimed.claim_token = Some("secret-a".into());
    let mut rival = claimed.clone();
    rival.claim_token = Some("secret-b".into());
    let (a, b) = tokio::join!(
        local::claim(&db, &expected, &claimed, 30, 3600),
        local::claim(&db, &expected, &rival, 30, 3600)
    );
    assert_ne!(a.unwrap(), b.unwrap());
    let mut claimed = local::load(&db, &original.id).await.unwrap().unwrap();
    let token = claimed.claim_token.clone().unwrap();
    assert!(local::require(&db, &claimed.id, "wrong").await.is_err());
    local::renew(&db, &claimed, &token, 120, 3600)
        .await
        .unwrap();
    db.close().await;
    let db = LocalRuntime::open(&root).await.unwrap();
    assert!(local::pending(&db, 10).await.unwrap().is_empty());
    assert_eq!(
        local::load(&db, &claimed.id)
            .await
            .unwrap()
            .unwrap()
            .claim_token
            .as_deref(),
        Some(token.as_str())
    );
    sqlx::query("UPDATE refill_tasks SET lease_until = 0 WHERE id = ?")
        .bind(&claimed.id)
        .execute(&db.pool)
        .await
        .unwrap();
    assert!(local::renew(&db, &claimed, &token, 30, 3600).await.is_err());
    assert_eq!(local::pending(&db, 10).await.unwrap().len(), 1);
    let expected = serialize_task(&claimed).unwrap();
    claimed.claim_token = Some("new-owner".into());
    assert!(local::claim(&db, &expected, &claimed, 30, 3600)
        .await
        .unwrap());
    claimed.state = CredentialRefillTaskState::Succeeded;
    claimed.claim_token = None;
    assert!(local::finish(&db, &claimed, &token, 3600).await.is_err());
    local::finish(&db, &claimed, "new-owner", 3600)
        .await
        .unwrap();
    assert!(local::pending(&db, 10).await.unwrap().is_empty());
    assert!(local::outstanding(&db, "a").await.unwrap().is_none());
    assert!(
        !local::create(&db, task("a", "key"), 3600)
            .await
            .unwrap()
            .created
    );
    assert!(
        local::create(&db, task("a", "another"), 3600)
            .await
            .unwrap()
            .created
    );
    let rows = local::list(
        &db,
        Some("a"),
        Some(CredentialRefillTaskState::Succeeded),
        10,
    )
    .await
    .unwrap();
    assert_eq!(rows.len(), 1);
    let public = serde_json::to_string(&rows).unwrap();
    assert!(!public.contains("claimToken"));
    assert!(!public.contains("idempotencyKey"));
    cleanup(root, db).await;
}
