//! Provider aliases preserve a key's principal, never its unrelated neighbors.
use super::{tests::input, tests::setup, *};
use crate::routing::config::RouteConfigStore;

fn routes(provider: &str) -> RouteConfigStore {
    RouteConfigStore::from_document(serde_json::from_value(serde_json::json!({
        "providers": [
            {"id":provider,"base_url":"https://example.invalid","supported_models":["public-model"],
             "credentials":[{"id":"account-one","api_key":"synthetic-only"}]},
            {"id":"unrelated","base_url":"https://example.invalid","supported_models":["public-model","private-model"],
             "credentials":[{"id":"account-other","api_key":"synthetic-only"}]}
        ],
        "account_groups":[{"id":"other-group","name":"Other", "provider_credential_ids":["account-other"]}]
    })).unwrap()).unwrap()
}

async fn key(db: &LocalRuntime, metadata: serde_json::Value) -> GatewayAccessKeyView {
    let mut value = input();
    value.metadata = Some(metadata);
    db.save_access_key(None, value).await.unwrap()
}

async fn protected(db: &LocalRuntime) -> (Vec<(String, String, String)>, Vec<(String, String)>) {
    (
        sqlx::query_as("SELECT id,token_hash,payload FROM local_access_keys ORDER BY id")
            .fetch_all(&db.pool)
            .await
            .unwrap(),
        sqlx::query_as("SELECT key_id,sealed FROM local_access_key_secrets ORDER BY key_id")
            .fetch_all(&db.pool)
            .await
            .unwrap(),
    )
}

async fn finish(root: std::path::PathBuf, db: LocalRuntime) {
    db.close().await;
    crate::local_runtime::test_support::remove_test_root(&root).await;
}

#[tokio::test]
async fn waiting_identity_snapshot_is_immutable_and_rejects_corrupt_registry() {
    let (root, db) = setup().await;
    crate::local_runtime::model_identity::rename(&db, "old", "new")
        .await
        .unwrap();
    let snapshot = db.provider_identity_snapshot().await.unwrap();
    crate::local_runtime::model_identity::rename(&db, "other", "other-api")
        .await
        .unwrap();
    assert_eq!(snapshot.counterpart("old"), Some("new"));
    assert_eq!(snapshot.counterpart("other"), None);
    assert_eq!(
        db.provider_identity_snapshot()
            .await
            .unwrap()
            .counterpart("other"),
        Some("other-api")
    );
    sqlx::query("INSERT INTO model_provider_aliases(alias,canonical) VALUES ('new','third')")
        .execute(&db.pool)
        .await
        .unwrap();
    assert!(db.provider_identity_snapshot().await.is_err());
    assert_eq!(snapshot.counterpart("old"), Some("new"));
    finish(root, db).await;
}

#[tokio::test]
async fn aliases_keep_discovery_request_and_fallback_consistent_without_rewriting_keys() {
    for (key_id, provider_id) in [("old", "new"), ("new", "old")] {
        let (root, db) = setup().await;
        let key = key(
            &db,
            serde_json::json!({"providerIds":[key_id],"models":["public-model"]}),
        )
        .await;
        let before = protected(&db).await;
        let routes = routes(provider_id);
        crate::local_runtime::model_identity::rename(&db, "old", "new")
            .await
            .unwrap();
        // Another alias must not become part of this key's fallback grant.
        crate::local_runtime::model_identity::rename(&db, "other", "unrelated")
            .await
            .unwrap();
        let models = db.local_access_models(&key.id, &routes).await.unwrap();
        assert_eq!(
            models.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(),
            ["public-model"]
        );
        let mut candidates = routes.resolve_candidates(Some("public-model"));
        assert_eq!(candidates.len(), 2);
        let allowed = db
            .authorize_local_candidates_with_fallback_authorization(
                &key.id,
                Some("public-model"),
                &mut candidates,
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(allowed.len(), 2);
        assert!(allowed.contains(&"old".into()) && allowed.contains(&"new".into()));
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].provider_account_id, provider_id);
        // Static route credentials live in payload; the database-owned field stays absent.
        assert!(candidates[0].provider_credential_id.is_none());
        assert_eq!(
            candidates[0].payload.credential_id.as_deref(),
            Some("account-one")
        );
        assert_eq!(before, protected(&db).await);
        let mut wrong_model = routes.resolve_candidates(Some("private-model"));
        assert!(db
            .authorize_local_candidates(&key.id, Some("private-model"), &mut wrong_model)
            .await
            .is_err());
        db.close().await;
        let db = LocalRuntime::open(&root).await.unwrap();
        assert_eq!(
            db.local_access_models(&key.id, &routes)
                .await
                .unwrap()
                .len(),
            1
        );
        assert_eq!(before, protected(&db).await);
        finish(root, db).await;
    }
}

#[tokio::test]
async fn absent_empty_and_unrelated_provider_restrictions_keep_their_meaning() {
    let (root, db) = setup().await;
    let routes = routes("new");
    crate::local_runtime::model_identity::rename(&db, "old", "new")
        .await
        .unwrap();
    for metadata in [
        serde_json::json!({"providerIds":[]}),
        serde_json::json!({"providerIds":["someone-else"]}),
    ] {
        let key = key(&db, metadata).await;
        assert!(db
            .local_access_models(&key.id, &routes)
            .await
            .unwrap()
            .is_empty());
        let mut candidates = routes.resolve_candidates(Some("public-model"));
        assert!(db
            .authorize_local_candidates(&key.id, Some("public-model"), &mut candidates)
            .await
            .is_err());
        assert!(candidates.is_empty());
    }
    let key = key(&db, serde_json::json!({})).await;
    assert_eq!(
        db.local_access_models(&key.id, &routes)
            .await
            .unwrap()
            .len(),
        2
    );
    let mut candidates = routes.resolve_candidates(Some("public-model"));
    assert!(db
        .authorize_local_candidates_with_fallback_authorization(
            &key.id,
            Some("public-model"),
            &mut candidates
        )
        .await
        .unwrap()
        .is_none());
    assert_eq!(candidates.len(), 2);
    finish(root, db).await;
}

#[tokio::test]
async fn provider_aliases_do_not_override_groups_admission_or_revocation() {
    let (root, db) = setup().await;
    let routes = routes("new");
    crate::local_runtime::model_identity::rename(&db, "old", "new")
        .await
        .unwrap();
    let grouped = key(
        &db,
        serde_json::json!({"providerIds":["old"],"accountGroupIds":["other-group"]}),
    )
    .await;
    assert!(db
        .local_access_models(&grouped.id, &routes)
        .await
        .unwrap()
        .is_empty());
    let key = key(&db, serde_json::json!({"providerIds":["old"]})).await;
    assert!(db
        .local_access_models_admitted(&key.id, &routes, &|_, _, _| false)
        .await
        .unwrap()
        .is_empty());
    db.revoke_access_key(&key.id, None).await.unwrap();
    assert!(db.local_access_models(&key.id, &routes).await.is_err());
    let mut candidates = routes.resolve_candidates(Some("public-model"));
    assert!(db
        .authorize_local_candidates(&key.id, Some("public-model"), &mut candidates)
        .await
        .is_err());
    finish(root, db).await;
}

#[tokio::test]
async fn malformed_registry_fails_closed_in_discovery_and_authorization() {
    let (root, db) = setup().await;
    let routes = routes("new");
    let key = key(&db, serde_json::json!({"providerIds":["old"]})).await;
    sqlx::query("INSERT INTO model_provider_aliases(alias,canonical) VALUES ('old','new'),('new','unrelated')")
        .execute(&db.pool).await.unwrap();
    assert!(db.local_access_models(&key.id, &routes).await.is_err());
    let mut candidates = routes.resolve_candidates(Some("public-model"));
    assert!(db
        .authorize_local_candidates(&key.id, Some("public-model"), &mut candidates)
        .await
        .is_err());
    finish(root, db).await;
}
