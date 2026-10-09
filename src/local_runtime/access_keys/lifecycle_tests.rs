//! Toggle state without resetting identity, quota or expiry; terminal tokens stay terminal.
use super::{
    tests::{input, request, setup},
    *,
};
use crate::access_balance::quota::KeyQuotaInput;

#[tokio::test]
async fn disable_enable_preserves_identity_quota_and_restart() {
    let (root, db) = setup().await;
    let key = db
        .save_access_key_with_quota(
            None,
            input(),
            Some(&KeyQuotaInput {
                mode: "message_prepaid".into(),
                limit: Some(5),
                currency: None,
            }),
        )
        .await
        .unwrap();
    let before = db.access_balance(&key.id).await.unwrap().unwrap();
    db.set_access_key_enabled(&key.id, false).await.unwrap();
    db.set_access_key_enabled(&key.id, false).await.unwrap();
    assert!(db.authenticate_access_key(&request(&key)).await.is_err());
    assert!(db.access_key_secret(&key.id).await.is_err());
    assert!(db.rotate_access_key(&key.id).await.is_err());
    db.close().await;
    let db = LocalRuntime::open(&root).await.unwrap();
    assert_eq!(
        db.access_catalog().await.unwrap().access_keys[0].status,
        "disabled"
    );
    db.set_access_key_enabled(&key.id, true).await.unwrap();
    assert!(db
        .authenticate_access_key(&request(&key))
        .await
        .unwrap()
        .is_some());
    assert_eq!(
        db.access_key_secret(&key.id).await.unwrap(),
        key.token.clone().unwrap()
    );
    assert_eq!(
        db.access_balance(&key.id)
            .await
            .unwrap()
            .unwrap()
            .remaining_messages,
        before.remaining_messages
    );
    let rotated = db.rotate_access_key(&key.id).await.unwrap();
    assert!(db.set_access_key_enabled(&key.id, true).await.is_err());
    db.revoke_access_key(&rotated.id, None).await.unwrap();
    assert!(db.set_access_key_enabled(&rotated.id, true).await.is_err());
    db.delete_access_key(&rotated.id).await.unwrap();
    assert!(db.set_access_key_enabled(&rotated.id, true).await.is_err());
    assert!(db
        .authenticate_access_key(&request(&rotated))
        .await
        .unwrap()
        .is_none());
    db.close().await;
    crate::local_runtime::test_support::remove_test_root(&root).await;
}

#[tokio::test]
async fn enabling_does_not_extend_expiry() {
    let (root, db) = setup().await;
    let mut expired = input();
    expired.expires_at = Some("2000-01-01T00:00:00Z".into());
    let key = db.save_access_key(None, expired).await.unwrap();
    db.set_access_key_enabled(&key.id, false).await.unwrap();
    db.set_access_key_enabled(&key.id, true).await.unwrap();
    assert!(db.authenticate_access_key(&request(&key)).await.is_err());
    assert_eq!(
        db.access_catalog().await.unwrap().access_keys[0].expires_at,
        key.expires_at
    );
    db.close().await;
    crate::local_runtime::test_support::remove_test_root(&root).await;
}
