//! Cache publication and restart repair require the original DIM site, subject and refresh material.
use super::dim_fixture::*;
use crate::console::refresh_grants::GrantRecord;
use serde_json::{json, Value};

#[tokio::test]
async fn dim_cache_repair_keeps_quotas_and_rejects_subject_site_mode_or_refresh_drift() {
    let f = fixture().await;
    let request = request(&f).1;
    let mut next = context();
    next["granted_scopes"] = json!(["openid"]);
    let result = request
        .result_with_dim(
            "new-access",
            Some("new-refresh"),
            3600,
            "2090-01-01T00:00:00Z",
            None,
            None,
            None,
            Some(serde_json::from_value(next.clone()).unwrap()),
        )
        .unwrap();
    let record = GrantRecord::sending(&request).with_result(&result).unwrap();
    let snapshot = f.runtime.route_config().snapshot();
    let payload = &snapshot
        .oauth_refresh_target("oauth", "account")
        .unwrap()
        .1
        .payload;
    let raw = json!({"id":"account","kind":"account_credential","provider":"openai",
        "project_id":"internal-project","user_id":"internal-user","api_key":"old-access","api_base_url":payload.base_url,
        "created_at":"2020-01-01T00:00:00Z","updated_at":"2020-01-01T00:00:00Z","expires_at":"2020-01-01T00:00:00Z",
        "quota_remaining_tokens":42,"account_payload":{"auth_mode":"dim_user_oauth","dim_oauth":context(),
        "refresh_token":"old-refresh","expires_at":"2020-01-01T00:00:00Z"}});
    let patch = |value: &Value| {
        super::super::server_cache::patch_from_grant(&value.to_string(), &record, payload).unwrap()
    };
    let (updated, ttl) = patch(&raw).unwrap();
    assert!(ttl > 0);
    let updated: Value = serde_json::from_str(&updated).unwrap();
    assert_eq!(updated["quota_remaining_tokens"], 42);
    assert_eq!(updated["account_payload"]["dim_oauth"], next);
    assert_eq!(updated["account_payload"]["refresh_token"], "new-refresh");
    assert!(patch(&updated).is_none());
    for (pointer, value) in [
        ("/api_base_url", json!("https://dimagent.com/v1")),
        ("/account_payload/auth_mode", json!("bearer")),
        ("/account_payload/dim_oauth/subject", json!("other-subject")),
        ("/account_payload/dim_oauth/nonce", json!("other-nonce")),
        ("/account_payload/refresh_token", Value::Null),
    ] {
        let mut raw = raw.clone();
        *raw.pointer_mut(pointer).unwrap() = value;
        assert!(patch(&raw).is_none(), "{pointer}");
    }
}
#[tokio::test]
async fn dim_normal_cache_cas_matches_context_and_preserves_unrelated_fields() {
    let witness = super::cache_wire::Cache::new(true).await;
    let raw = json!({"id":"exact-build","api_key":"old-access","api_base_url":"https://dimagent.cn/v1","quota_remaining_tokens":42,
        "account_payload":{"auth_mode":"dim_user_oauth","dim_oauth":context(),"refresh_token":"old-refresh"}});
    witness.replace_raw(raw.clone());
    let original = serde_json::from_value(context()).unwrap();
    let mut next = context();
    next["granted_scopes"] = json!(["openid"]);
    let token = super::super::transport::TokenResponse {
        access_token: "new-access".into(),
        refresh_token: Some("new-refresh".into()),
        expires_in: Some(3600),
        absolute_expiry: None,
        grok_oauth: None,
        vertex_oauth: None,
        accio_sgcookie: None,
        dim_oauth: Some(serde_json::from_value(next.clone()).unwrap()),
        nous_portal: None,
        lobsterai: None,
    };
    super::super::server_cache::persist(
        &witness.pool,
        "exact-build",
        "old-access",
        "old-refresh",
        None,
        None,
        None,
        Some(&original),
        None,
        &token,
        "2090-01-01T00:00:00Z",
    )
    .await
    .unwrap();
    assert_eq!(witness.commits(), 1);
    assert_eq!(witness.value()["account_payload"]["dim_oauth"], next);
    assert_eq!(witness.value()["quota_remaining_tokens"], 42);
    witness.stop().await;
}
