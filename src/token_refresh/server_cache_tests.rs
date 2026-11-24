use super::*;

#[test]
fn cache_patch_preserves_quotas_and_rejects_reauthorization() {
    let raw = json!({"api_key":"old", "quota_remaining_tokens":42,
        "unknown_field":"retained", "account_payload":{"refresh_token":"old-rt",
        "session_auth":{"expires_at":"old"}, "headers":{"x-id":"account"}}})
    .to_string();
    let token = TokenResponse {
        grok_oauth: None,
        absolute_expiry: None,
        accio_sgcookie: None,
        vertex_oauth: None,
        dim_oauth: None,
        nous_portal: None,
        lobsterai: None,
        access_token: "new".into(),
        refresh_token: Some("new-rt".into()),
        expires_in: Some(3600),
    };
    let patched = patch(
        &raw,
        "old",
        "old-rt",
        None,
        None,
        None,
        None,
        None,
        &token,
        "new-expiry",
    )
    .unwrap();
    let value: Value = serde_json::from_str(&patched).unwrap();
    assert_eq!(value["quota_remaining_tokens"], 42);
    assert_eq!(value["unknown_field"], "retained");
    assert_eq!(value["account_payload"]["refresh_token"], "new-rt");
    assert_eq!(
        value["account_payload"]["session_auth"]["expires_at"],
        "new-expiry"
    );
    assert!(patch(
        &patched,
        "old",
        "old-rt",
        None,
        None,
        None,
        None,
        None,
        &token,
        "new-expiry"
    )
    .is_none());
    assert!(patch(
        &raw,
        "old",
        "other-rt",
        None,
        None,
        None,
        None,
        None,
        &token,
        "new-expiry"
    )
    .is_none());
}

#[test]
fn grok_cache_patch_cas_binds_original_context_and_persists_scope_narrowing() {
    let value = json!({"issuer":"https://auth.x.ai","client_id":"explicit-client",
        "requested_scopes":["openid","profile"],"granted_scopes":["openid","profile"]});
    let context: crate::protocol::grok_oauth::AuthorizationContext =
        serde_json::from_value(value.clone()).unwrap();
    let mut narrowed = value;
    narrowed["granted_scopes"] = json!(["profile"]);
    let next: crate::protocol::grok_oauth::AuthorizationContext =
        serde_json::from_value(narrowed).unwrap();
    let raw = json!({"api_key":"old","account_payload":{"refresh_token":"old-rt",
        "grok_oauth":context},"quota_remaining_tokens":42})
    .to_string();
    let token = TokenResponse {
        access_token: "new".into(),
        refresh_token: Some("new-rt".into()),
        expires_in: Some(3600),
        grok_oauth: Some(next.clone()),
        absolute_expiry: None,
        accio_sgcookie: None,
        vertex_oauth: None,
        dim_oauth: None,
        nous_portal: None,
        lobsterai: None,
    };
    let patched = patch(
        &raw,
        "old",
        "old-rt",
        Some(&context),
        None,
        None,
        None,
        None,
        &token,
        "new-expiry",
    )
    .unwrap();
    let stored: Value = serde_json::from_str(&patched).unwrap();
    assert_eq!(
        stored["account_payload"]["grok_oauth"],
        serde_json::to_value(&next).unwrap()
    );
    assert_eq!(stored["quota_remaining_tokens"], 42);
    assert!(patch(
        &raw,
        "old",
        "old-rt",
        None,
        None,
        None,
        None,
        None,
        &token,
        "new-expiry"
    )
    .is_none());
    assert!(patch(
        &raw,
        "old",
        "old-rt",
        Some(&next),
        None,
        None,
        None,
        None,
        &token,
        "new-expiry"
    )
    .is_none());
    assert!(patch(
        &patched,
        "new",
        "new-rt",
        Some(&context),
        None,
        None,
        None,
        None,
        &token,
        "later-expiry"
    )
    .is_none());
    assert!(patch(
        &patched,
        "new",
        "new-rt",
        Some(&next),
        None,
        None,
        None,
        None,
        &token,
        "later-expiry"
    )
    .is_some());
}
