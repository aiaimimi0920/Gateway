//! Upgrade, disclosure and quota edits exercise the real SQLite transactions.
use super::{
    tests::{input, request, setup},
    *,
};
use crate::access_balance::{quota::KeyQuotaInput, AccessBalanceStore};

fn quota(mode: &str, limit: Option<i64>) -> KeyQuotaInput {
    KeyQuotaInput {
        mode: mode.into(),
        limit,
        currency: None,
    }
}

#[tokio::test]
async fn managed_copy_survives_restart_without_catalog_or_database_plaintext() {
    let (root, db) = setup().await;
    let key = db.save_access_key(None, input()).await.unwrap();
    let token = key.token.as_deref().unwrap();
    assert_eq!(db.access_key_secret(&key.id).await.unwrap(), token);
    assert_eq!(db.access_key_secret(&key.id).await.unwrap(), token);
    let sealed: String =
        sqlx::query_scalar("SELECT sealed FROM local_access_key_secrets WHERE key_id=?")
            .bind(&key.id)
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert!(!sealed.contains(token));
    assert!(!serde_json::to_string(&db.access_catalog().await.unwrap())
        .unwrap()
        .contains(token));
    db.close().await;
    let db = LocalRuntime::open(&root).await.unwrap();
    assert_eq!(db.access_key_secret(&key.id).await.unwrap(), token);
    let replacement = db.rotate_access_key(&key.id).await.unwrap();
    assert!(db.access_key_secret(&key.id).await.is_err());
    assert_eq!(
        db.access_key_secret(&replacement.id).await.unwrap(),
        replacement.token.unwrap()
    );
    db.revoke_access_key(&replacement.id, None).await.unwrap();
    assert!(db.access_key_secret(&replacement.id).await.is_err());
    db.delete_access_key(&replacement.id).await.unwrap();
    let remaining: i64 =
        sqlx::query_scalar("SELECT count(*) FROM local_access_key_secrets WHERE key_id=?")
            .bind(&replacement.id)
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert_eq!(remaining, 0);
    db.close().await;
    crate::local_runtime::test_support::remove_test_root(&root).await;
}

#[tokio::test]
async fn legacy_hash_only_keys_capture_after_verified_use_without_rotation() {
    let (root, db) = setup().await;
    let key = db.save_access_key(None, input()).await.unwrap();
    sqlx::query("DELETE FROM local_access_key_secrets")
        .execute(&db.pool)
        .await
        .unwrap();
    db.close().await;
    let db = LocalRuntime::open(&root).await.unwrap();
    assert_eq!(
        db.access_key_secret(&key.id)
            .await
            .unwrap_err()
            .code
            .as_deref(),
        Some("access_key_secret_not_saved")
    );
    let mut bad = request(&key);
    bad.authorization = Some("Bearer invalid-fixture".into());
    assert!(db.authenticate_access_key(&bad).await.unwrap().is_none());
    assert!(db.access_key_secret(&key.id).await.is_err());
    assert!(db
        .authenticate_access_key(&request(&key))
        .await
        .unwrap()
        .is_some());
    assert_eq!(
        db.access_key_secret(&key.id).await.unwrap(),
        key.token.unwrap()
    );
    assert_eq!(db.access_catalog().await.unwrap().access_keys[0].id, key.id);
    db.close().await;
    crate::local_runtime::test_support::remove_test_root(&root).await;
}

#[tokio::test]
async fn missing_seal_key_fails_closed_but_authentication_and_recovery_preserve_tokens() {
    let (root, db) = setup().await;
    let key = db.save_access_key(None, input()).await.unwrap();
    let path = root.join("access-key-seal.json");
    let original = std::fs::read(&path).unwrap();
    db.close().await;
    std::fs::remove_file(&path).unwrap();
    let db = LocalRuntime::open(&root).await.unwrap();
    assert!(!path.exists());
    assert!(db
        .authenticate_access_key(&request(&key))
        .await
        .unwrap()
        .is_some());
    assert!(db.access_key_secret(&key.id).await.is_err());
    assert!(db.rotate_access_key(&key.id).await.is_err());
    assert!(db.save_access_key(None, input()).await.is_err());
    assert_eq!(db.access_catalog().await.unwrap().access_keys.len(), 1);
    assert_eq!(
        db.access_catalog().await.unwrap().access_keys[0].status,
        "active"
    );
    db.close().await;
    std::fs::write(&path, original).unwrap();
    let db = LocalRuntime::open(&root).await.unwrap();
    assert_eq!(
        db.access_key_secret(&key.id).await.unwrap(),
        key.token.unwrap()
    );
    db.close().await;
    crate::local_runtime::test_support::remove_test_root(&root).await;
}

#[tokio::test]
async fn swapped_sealed_payloads_cannot_disclose_another_key() {
    let (root, db) = setup().await;
    let first = db.save_access_key(None, input()).await.unwrap();
    let second = db.save_access_key(None, input()).await.unwrap();
    sqlx::query("UPDATE local_access_key_secrets SET sealed=(SELECT sealed FROM local_access_key_secrets WHERE key_id=?) WHERE key_id=?")
        .bind(&first.id).bind(&second.id).execute(&db.pool).await.unwrap();
    assert!(db.access_key_secret(&second.id).await.is_err());
    assert!(db
        .authenticate_access_key(&request(&second))
        .await
        .unwrap()
        .is_some());
    db.close().await;
    crate::local_runtime::test_support::remove_test_root(&root).await;
}

#[tokio::test]
async fn total_quota_edits_preserve_spend_holds_rotation_and_atomic_failure() {
    let (root, db) = setup().await;
    let key = db
        .save_access_key_with_quota(None, input(), Some(&quota("message_prepaid", Some(5))))
        .await
        .unwrap();
    let balances = AccessBalanceStore::Sqlite(&db);
    assert!(balances.reserve(&key.id, 1).await.unwrap().allowed);
    balances.settle(&key.id, 1, 2).await.unwrap();
    assert!(balances.reserve(&key.id, 1).await.unwrap().allowed);
    let mut renamed = input();
    renamed.display_name = "renamed".into();
    db.save_access_key(Some(&key.id), renamed.clone())
        .await
        .unwrap();
    assert_eq!(
        balances
            .get(&key.id)
            .await
            .unwrap()
            .unwrap()
            .remaining_messages,
        Some(3)
    );
    db.save_access_key_with_quota(
        Some(&key.id),
        renamed.clone(),
        Some(&quota("message_prepaid", Some(10))),
    )
    .await
    .unwrap();
    assert_eq!(
        balances
            .get(&key.id)
            .await
            .unwrap()
            .unwrap()
            .remaining_messages,
        Some(8)
    );
    renamed.display_name = "must-not-save".into();
    assert!(db
        .save_access_key_with_quota(
            Some(&key.id),
            renamed.clone(),
            Some(&quota("token_prepaid", Some(500)))
        )
        .await
        .is_err());
    assert_eq!(
        db.access_catalog().await.unwrap().access_keys[0].display_name,
        "renamed"
    );
    balances.refund(&key.id, 1).await.unwrap();
    assert_eq!(
        balances
            .get(&key.id)
            .await
            .unwrap()
            .unwrap()
            .remaining_messages,
        Some(9)
    );
    db.save_access_key_with_quota(
        Some(&key.id),
        input(),
        Some(&quota("message_prepaid", Some(0))),
    )
    .await
    .unwrap();
    assert!(!balances.reserve(&key.id, 1).await.unwrap().allowed);
    assert_eq!(
        balances
            .get(&key.id)
            .await
            .unwrap()
            .unwrap()
            .remaining_messages,
        Some(-1)
    );
    let replacement = db.rotate_access_key(&key.id).await.unwrap();
    assert_eq!(
        balances
            .get(&replacement.id)
            .await
            .unwrap()
            .unwrap()
            .remaining_messages,
        Some(-1)
    );
    db.close().await;
    let db = LocalRuntime::open(&root).await.unwrap();
    assert_eq!(
        db.access_balance(&replacement.id)
            .await
            .unwrap()
            .unwrap()
            .remaining_messages,
        Some(-1)
    );
    db.save_access_key_with_quota(
        Some(&replacement.id),
        input(),
        Some(&quota("token_prepaid", Some(100))),
    )
    .await
    .unwrap();
    let balances = AccessBalanceStore::Sqlite(&db);
    assert!(balances.reserve(&replacement.id, 20).await.unwrap().allowed);
    db.save_access_key_with_quota(
        Some(&replacement.id),
        input(),
        Some(&quota("token_prepaid", Some(150))),
    )
    .await
    .unwrap();
    balances.settle(&replacement.id, 20, 7).await.unwrap();
    assert_eq!(
        balances
            .get(&replacement.id)
            .await
            .unwrap()
            .unwrap()
            .remaining_tokens,
        Some(143)
    );
    assert!(db
        .save_access_key_with_quota(None, input(), Some(&quota("token_prepaid", Some(-1))))
        .await
        .is_err());
    assert_eq!(db.access_catalog().await.unwrap().access_keys.len(), 2);
    db.close().await;
    crate::local_runtime::test_support::remove_test_root(&root).await;
}
