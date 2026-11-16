//! Normal and restart cache patches compare the exact original immutable Kiro registration.
use super::{kiro_fixture::*, *};
use crate::console::refresh_grants::*;
use serde_json::{json, Value};

#[tokio::test]
async fn kiro_cache_repair_rejects_changed_registration_but_preserves_quota_and_unknown_fields() {
    let fixture = fixture().await;
    let request = request(&fixture).1;
    let result = request
        .result(
            "new-access",
            Some("new-refresh"),
            1,
            "2021-01-01T00:00:00Z",
            None,
        )
        .unwrap();
    let record = GrantRecord::sending(&request).with_result(&result).unwrap();
    let snapshot = fixture.runtime.route_config().snapshot();
    let target = &snapshot
        .oauth_refresh_target("oauth", "account")
        .unwrap()
        .1
        .payload;
    let patch = |value: &Value| {
        super::super::server_cache::patch_from_grant(&value.to_string(), &record, target).unwrap()
    };
    let original = cache();
    let (patched, ttl) = patch(&original).unwrap();
    let patched: Value = serde_json::from_str(&patched).unwrap();
    assert_eq!(ttl, 1);
    assert_eq!(patched["account_payload"]["kiro_oauth"], context());
    assert_eq!(patched["api_key"], "new-access");
    assert_eq!(patched["quota_remaining_tokens"], 42);
    assert_eq!(patched["unknown"], "retained");
    assert_eq!(patched["expires_at"], "2021-01-01T00:00:00Z");
    assert!(patch(&patched).is_none());
    for (field, value) in [
        ("client_secret", json!("other-secret")),
        ("machine_id", json!("b".repeat(64))),
        ("scopes", json!(["codewhisperer:analysis"])),
        ("client_secret_expires_at", json!(1893456001i64)),
    ] {
        let mut changed = original.clone();
        changed["account_payload"]["kiro_oauth"][field] = value;
        assert!(patch(&changed).is_none(), "accepted changed {field}");
    }
    for invalid in [Value::Null, json!(false), json!(123), json!({}), json!([])] {
        let mut changed = original.clone();
        changed["account_payload"]["kiro_oauth"] = invalid;
        assert!(patch(&changed).is_none());
    }
}

#[tokio::test]
async fn kiro_normal_cache_cas_binds_old_context_and_keeps_registration_unchanged() {
    let witness = super::cache_wire::Cache::new(true).await;
    let mut raw = cache();
    raw["id"] = json!("exact-build");
    witness.replace_raw(raw.clone());
    let token = super::super::transport::TokenResponse {
        access_token: "new-access".into(),
        refresh_token: Some("new-refresh".into()),
        expires_in: Some(3600),
        grok_oauth: None,
        absolute_expiry: Some("2099-01-01T00:00:00Z".into()),
        accio_sgcookie: None,
        vertex_oauth: None,
        dim_oauth: None,
        nous_portal: None,
        lobsterai: None,
    };
    let original: crate::protocol::kiro_oauth::AuthorizationContext =
        serde_json::from_value(context()).unwrap();
    let mut missing_refresh = raw.clone();
    missing_refresh["account_payload"]
        .as_object_mut()
        .unwrap()
        .remove("refresh_token");
    witness.replace_raw(missing_refresh.clone());
    super::super::server_cache::persist(
        &witness.pool,
        "exact-build",
        "old-access",
        "old-refresh",
        None,
        Some(&original),
        None,
        None,
        None,
        &token,
        "2099-01-01T00:00:00Z",
    )
    .await
    .unwrap();
    assert_eq!(
        witness.commits(),
        0,
        "a cleared refresh token cannot be restored from stale work"
    );
    assert_eq!(witness.value(), missing_refresh);
    witness.replace_raw(raw);
    let mut changed = context();
    changed["client_secret"] = json!("other-secret");
    let changed = serde_json::from_value(changed).unwrap();
    for (expected, commits) in [(&changed, 0), (&original, 1)] {
        super::super::server_cache::persist(
            &witness.pool,
            "exact-build",
            "old-access",
            "old-refresh",
            None,
            Some(expected),
            None,
            None,
            None,
            &token,
            "2099-01-01T00:00:00Z",
        )
        .await
        .unwrap();
        assert_eq!(witness.commits(), commits);
    }
    assert_eq!(witness.value()["account_payload"]["kiro_oauth"], context());
    assert_eq!(witness.value()["quota_remaining_tokens"], 42);
    witness.stop().await;
}
