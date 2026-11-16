//! Cache projections bind the whole original Vertex context and product; Redis Lua is unchanged.
use super::{vertex_fixture::*, *};
use crate::console::refresh_grants::GrantRecord;
use serde_json::{json, Value};

#[tokio::test]
async fn vertex_oauth_cache_repair_persists_mutable_context_and_rejects_audience_drift() {
    let f = fixture().await;
    let original = request(&f).1;
    let mut next = context();
    next["scope_reported"] = json!(true);
    next["refresh_expires_at"] = json!("2080-01-01T00:00:00.123456789Z");
    let result = original
        .result_with_vertex(
            "new-access",
            Some("new-refresh"),
            3600,
            "2090-01-01T00:00:00Z",
            None,
            None,
            Some(serde_json::from_value(next.clone()).unwrap()),
        )
        .unwrap();
    let record = GrantRecord::sending(&original)
        .with_result(&result)
        .unwrap();
    let snapshot = f.runtime.route_config().snapshot();
    let payload = &snapshot
        .oauth_refresh_target("oauth", "account")
        .unwrap()
        .1
        .payload;
    let raw = json!({"id":"account","kind":"account_credential","provider":"vertex",
        "project_id":"internal-project","user_id":"internal-user","api_key":"old-access", "api_base_url":payload.base_url,
        "created_at":"2020-01-01T00:00:00Z","updated_at":"2020-01-01T00:00:00Z",
        "expires_at":"2020-01-01T00:00:00Z", "quota_remaining_tokens":42,
        "account_payload":{"preset":"google-agent-platform-official-api", "auth_mode":"vertex_user_oauth",
        "vertex_oauth":context(),"refresh_token":"old-refresh","expires_at":"2020-01-01T00:00:00Z"}});
    let patch = |raw: &Value| {
        super::super::server_cache::patch_from_grant(&raw.to_string(), &record, payload).unwrap()
    };
    let (updated, ttl) = patch(&raw).unwrap();
    assert!(ttl > 0);
    let updated: Value = serde_json::from_str(&updated).unwrap();
    assert_eq!(updated["account_payload"]["vertex_oauth"], next);
    assert_eq!(updated["quota_remaining_tokens"], 42);
    assert_eq!(updated["account_payload"]["refresh_token"], "new-refresh");
    assert!(patch(&updated).is_none());
    for (pointer, changed) in [
        (
            "/account_payload/auth_mode",
            json!("vertex_express_api_key"),
        ),
        (
            "/account_payload/vertex_oauth/target/project",
            json!("other-project"),
        ),
        (
            "/account_payload/vertex_oauth/target/quota_project",
            json!("other-quota"),
        ),
        (
            "/account_payload/vertex_oauth/client_secret",
            json!("other-private-client"),
        ),
        (
            "/account_payload/vertex_oauth/refresh_expires_at",
            json!("2099-01-02T00:00:00Z"),
        ),
        ("/account_payload/refresh_token", Value::Null),
    ] {
        let mut changed_raw = raw.clone();
        *changed_raw.pointer_mut(pointer).unwrap() = changed;
        assert!(patch(&changed_raw).is_none(), "{pointer}");
    }
    let mut missing_mode = raw.clone();
    missing_mode["account_payload"]
        .as_object_mut()
        .unwrap()
        .remove("auth_mode");
    assert!(patch(&missing_mode).is_none());
}

#[tokio::test]
async fn vertex_oauth_cache_writeback_uses_exact_original_context_cas() {
    let f = fixture().await;
    let witness = super::cache_wire::Cache::new(true).await;
    let raw = json!({"id":"exact-build","api_key":"old","quota_remaining_tokens":42,
        "account_payload":{"auth_mode":"vertex_user_oauth", "vertex_oauth":context(),"refresh_token":"old-rt"}});
    witness.replace_raw(raw.clone());
    let original = f.saved().vertex_oauth.unwrap();
    let mut next = context();
    next["scope_reported"] = json!(true);
    let token = super::super::transport::TokenResponse {
        access_token: "new-access".into(),
        refresh_token: Some("new-refresh".into()),
        expires_in: Some(3600),
        absolute_expiry: None,
        grok_oauth: None,
        accio_sgcookie: None,
        vertex_oauth: Some(serde_json::from_value(next.clone()).unwrap()),
        dim_oauth: None,
        nous_portal: None,
        lobsterai: None,
    };
    super::super::server_cache::persist(
        &witness.pool,
        "exact-build",
        "old",
        "old-rt",
        None,
        None,
        Some(&original),
        None,
        None,
        &token,
        "2090-01-01T00:00:00Z",
    )
    .await
    .unwrap();
    assert_eq!(witness.commits(), 1);
    assert_eq!(witness.value()["account_payload"]["vertex_oauth"], next);
    assert_eq!(
        witness.value()["quota_remaining_tokens"],
        raw["quota_remaining_tokens"]
    );
    witness.stop().await;
}
