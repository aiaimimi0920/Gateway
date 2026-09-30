use super::*;
use crate::auth::adapter::AuthRequest;

fn input() -> UpsertAccessKeyInput {
    UpsertAccessKeyInput {
        owner_type: "user".into(),
        owner_id: "owner".into(),
        resolved_project_id: "local".into(),
        resolved_tenant_id: "local".into(),
        key_kind: "user".into(),
        public_key_prefix: "sk-gw".into(),
        display_name: "NVIDIA test".into(),
        expires_at: None,
        bundle_ids: vec![],
        metadata: Some(
            serde_json::json!({"scopes":["chat.completions"], "models":["nvidia-test"]}),
        ),
    }
}

fn request(key: &GatewayAccessKeyView) -> AuthRequest {
    AuthRequest {
        authorization: Some(format!("Bearer {}", key.token.as_deref().unwrap())),
        api_key: None,
        path: "/v1/chat/completions".into(),
        method: "POST".into(),
    }
}

async fn setup() -> (std::path::PathBuf, LocalRuntime) {
    let root = std::env::temp_dir().join(format!("gateway-local-keys-{}", uuid::Uuid::new_v4()));
    let runtime = LocalRuntime::open(&root).await.unwrap();
    (root, runtime)
}

#[tokio::test]
async fn local_keys_upgrade_schema_one_and_preserve_existing_data_and_secrets() {
    let (root, db) = setup().await;
    sqlx::query("DROP TABLE local_access_keys")
        .execute(&db.pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO provider_pricing VALUES ('preserved', 'unchanged')")
        .execute(&db.pool)
        .await
        .unwrap();
    db.close().await;
    let db = LocalRuntime::open(&root).await.unwrap();
    let key = db.save_access_key(None, input()).await.unwrap();
    let req = request(&key);
    let session = db.authenticate_access_key(&req).await.unwrap().unwrap();
    assert_eq!(session.access_key_id.as_deref(), Some(key.id.as_str()));
    let stored: String = sqlx::query_scalar("SELECT payload FROM local_access_keys WHERE id = ?")
        .bind(&key.id)
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert!(!stored.contains(key.token.as_deref().unwrap()));
    assert!(db.access_catalog().await.unwrap().access_keys[0]
        .token
        .is_none());
    db.close().await;
    let db = LocalRuntime::open(&root).await.unwrap();
    assert!(db.authenticate_access_key(&req).await.unwrap().is_some());
    let preserved: String =
        sqlx::query_scalar("SELECT payload FROM provider_pricing WHERE provider_id = 'preserved'")
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert_eq!(preserved, "unchanged");
    let version: i64 = sqlx::query_scalar("PRAGMA user_version")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(version, 1);
    db.close().await;
    crate::local_runtime::test_support::remove_test_root(&root).await;
}

#[tokio::test]
async fn local_keys_rotation_is_atomic_and_single_winner_then_revoke_delete_deny() {
    let (root, db) = setup().await;
    let key = db.save_access_key(None, input()).await.unwrap();
    let (a, b) = tokio::join!(db.rotate_access_key(&key.id), db.rotate_access_key(&key.id));
    assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
    let replacement = a.or(b).unwrap();
    assert_eq!(replacement.metadata, key.metadata);
    assert_eq!(
        replacement.rotated_from_access_key_id.as_deref(),
        Some(key.id.as_str())
    );
    assert!(db.authenticate_access_key(&request(&key)).await.is_err());
    assert!(db
        .authenticate_access_key(&request(&replacement))
        .await
        .unwrap()
        .is_some());
    db.revoke_access_key(&replacement.id, Some("test"))
        .await
        .unwrap();
    assert!(db
        .authenticate_access_key(&request(&replacement))
        .await
        .is_err());
    assert!(db
        .save_access_key(Some(&replacement.id), input())
        .await
        .is_err());
    db.delete_access_key(&replacement.id).await.unwrap();
    assert!(db
        .authenticate_access_key(&request(&replacement))
        .await
        .unwrap()
        .is_none());
    db.close().await;
    crate::local_runtime::test_support::remove_test_root(&root).await;
}

#[tokio::test]
async fn local_keys_expiry_scope_restrictions_and_invalid_policies_fail_closed() {
    let (root, db) = setup().await;
    let mut expired = input();
    expired.expires_at = Some("2020-01-01T00:00:00Z".into());
    let key = db.save_access_key(None, expired).await.unwrap();
    assert!(db.authenticate_access_key(&request(&key)).await.is_err());
    let key = db.save_access_key(None, input()).await.unwrap();
    let mut req = request(&key);
    req.path = "/v1/embeddings".into();
    assert!(db.authenticate_access_key(&req).await.is_err());
    assert!(db
        .authorize_local_candidates(&key.id, Some("forbidden"), &mut vec![])
        .await
        .is_err());
    for metadata in [
        serde_json::json!({"models":"*"}),
        serde_json::json!({"scopes": [1]}),
        serde_json::json!({"unknown": true}),
    ] {
        let mut invalid = input();
        invalid.metadata = Some(metadata);
        assert!(db.save_access_key(None, invalid).await.is_err());
    }
    let mut invalid = input();
    invalid.bundle_ids = vec!["not-local".into()];
    assert!(db.save_access_key(None, invalid).await.is_err());
    let mut invalid = input();
    invalid.expires_at = Some("invalid".into());
    assert!(db.save_access_key(None, invalid).await.is_err());
    db.close().await;
    crate::local_runtime::test_support::remove_test_root(&root).await;
}

#[tokio::test]
async fn local_keys_failed_rotation_rolls_back_revocation() {
    let (root, db) = setup().await;
    let key = db.save_access_key(None, input()).await.unwrap();
    sqlx::query("CREATE TRIGGER reject_key_insert BEFORE INSERT ON local_access_keys BEGIN SELECT RAISE(ABORT, 'test failure'); END")
        .execute(&db.pool).await.unwrap();
    assert!(db.rotate_access_key(&key.id).await.is_err());
    assert!(db
        .authenticate_access_key(&request(&key))
        .await
        .unwrap()
        .is_some());
    assert_eq!(db.access_catalog().await.unwrap().access_keys.len(), 1);
    db.close().await;
    crate::local_runtime::test_support::remove_test_root(&root).await;
}

#[tokio::test]
async fn local_keys_provider_and_model_limits_filter_discovery_and_routes() {
    let (root, db) = setup().await;
    let routes = crate::routing::config::RouteConfigStore::new();
    let document = serde_yaml::from_str(
        r#"
providers:
  - id: nvidia-allowed
    base_url: https://example.invalid
    api_key: test-only
    supported_models: [nvidia-test]
  - id: nvidia-denied
    base_url: https://example.invalid
    api_key: test-only
    supported_models: [nvidia-test, private-model]
model_routes: []
"#,
    )
    .unwrap();
    routes.replace_document(document).unwrap();
    let mut limited = input();
    limited.metadata =
        Some(serde_json::json!({"providerIds":["nvidia-allowed"], "models":["nvidia-test"]}));
    let key = db.save_access_key(None, limited).await.unwrap();
    let models = db.local_access_models(&key.id, &routes).await.unwrap();
    assert_eq!(models.len(), 1);
    assert_eq!(models[0].id, "nvidia-test");
    let mut candidates = routes.resolve_candidates(Some("nvidia-test"));
    assert_eq!(candidates.len(), 2);
    db.authorize_local_candidates(&key.id, Some("nvidia-test"), &mut candidates)
        .await
        .unwrap();
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].provider_account_id, "nvidia-allowed");
    db.revoke_access_key(&key.id, None).await.unwrap();
    assert!(db
        .authorize_local_candidates(&key.id, Some("nvidia-test"), &mut candidates)
        .await
        .is_err());
    assert!(db.local_access_models(&key.id, &routes).await.is_err());
    db.close().await;
    crate::local_runtime::test_support::remove_test_root(&root).await;
}
