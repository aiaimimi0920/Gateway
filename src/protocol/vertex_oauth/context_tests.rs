//! Context identity and deadline evidence are independent of live Google credentials.
use super::{
    AuthorizationContext, OAuthClient, OAuthError, TokenMaterial, VertexTarget,
    CLOUD_PLATFORM_SCOPE,
};
use serde_json::{json, Value};
use time::OffsetDateTime;

pub(super) fn value() -> Value {
    json!({"grant_type":"desktop_authorization_code", "client_id":"explicit-client", "client_secret":"synthetic-client-secret", "requested_scope":CLOUD_PLATFORM_SCOPE, "scope_reported":true, "target":{"project":"explicit-project","location":"global","quota_project":"billing-project"}, "refresh_expires_at":"2099-01-01T00:00:00.123456789Z"})
}
pub(super) fn context(value: Value) -> AuthorizationContext {
    serde_json::from_value(value).unwrap()
}
pub(super) fn material(deadline: Option<u64>, report: bool) -> TokenMaterial {
    let mut reply = json!({"access_token":"synthetic-access", "refresh_token":"synthetic-refresh", "token_type":"Bearer", "expires_in":3600});
    if let Some(seconds) = deadline {
        reply["refresh_token_expires_in"] = json!(seconds);
    }
    if report {
        reply["scope"] = json!(CLOUD_PLATFORM_SCOPE);
    }
    TokenMaterial::parse(
        &serde_json::to_vec(&reply).unwrap(),
        true,
        OffsetDateTime::now_utc(),
    )
    .unwrap()
}

#[test]
fn vertex_oauth_context_material_constructor_keeps_explicit_target_and_no_tokens() {
    let token = material(Some(7200), true);
    let client = OAuthClient::new("explicit-client", Some("synthetic-client-secret")).unwrap();
    let target =
        VertexTarget::new("explicit-project", "us-central1", Some("billing-project")).unwrap();
    let ctx = AuthorizationContext::from_authorization(client, target.clone(), &token).unwrap();
    assert_eq!(ctx.client_id(), "explicit-client");
    assert_eq!(ctx.client_secret(), Some("synthetic-client-secret"));
    assert_eq!(ctx.target(), &target);
    assert!(ctx.scope_reported());
    assert_eq!(
        ctx.refresh_expires_at().unwrap(),
        token.refresh_expires_at()
    );
    assert!(ctx.refresh_grant("synthetic-refresh").is_ok());
    let encoded = serde_json::to_string(&ctx).unwrap();
    assert!(!encoded.contains("synthetic-access"));
    assert!(!encoded.contains("synthetic-refresh"));
    assert!(!encoded.contains("verified_user"));
    assert_eq!(context(serde_json::from_str(&encoded).unwrap()), ctx);
    for secret in [
        "explicit-client",
        "synthetic-client-secret",
        "explicit-project",
    ] {
        assert!(!format!("{ctx:?}").contains(secret));
    }
}

#[test]
fn vertex_oauth_context_optional_public_client_secret_and_unreported_scope_are_not_guessed() {
    let token = material(None, false);
    let ctx = AuthorizationContext::from_authorization(
        OAuthClient::new("public-client", None).unwrap(),
        VertexTarget::new("explicit-project", "global", None).unwrap(),
        &token,
    )
    .unwrap();
    assert_eq!(ctx.client_secret(), None);
    assert!(!ctx.scope_reported());
    assert_eq!(ctx.refresh_expires_at().unwrap(), None);
    let stored = serde_json::to_value(ctx).unwrap();
    assert!(stored.get("client_secret").is_none());
    assert!(stored.get("refresh_expires_at").is_none());
    assert_eq!(stored["requested_scope"], CLOUD_PLATFORM_SCOPE);
}

#[test]
fn vertex_oauth_context_rejects_wrong_scope_target_and_invalid_stored_deadline() {
    for (pointer, replacement) in [
        ("/client_id", json!("")),
        ("/client_secret", json!("bad\r\nsecret")),
        ("/requested_scope", json!("openid")),
        (
            "/requested_scope",
            json!(format!("{CLOUD_PLATFORM_SCOPE} openid")),
        ),
        ("/target/project", json!("project/other")),
        ("/target/location", json!("evil.test")),
        ("/refresh_expires_at", json!("bad")),
        ("/refresh_expires_at", json!("1969-01-01T00:00:00Z")),
        ("/refresh_expires_at", json!("x".repeat(65))),
    ] {
        let mut bad = value();
        *bad.pointer_mut(pointer).unwrap() = replacement;
        let ctx = context(bad);
        assert_eq!(ctx.validate(), Err(OAuthError::InvalidInput), "{pointer}");
        assert!(ctx.refresh_grant("synthetic-refresh").is_err());
    }
}

#[test]
fn vertex_oauth_context_closed_schema_requires_explicit_grant_and_scope_evidence() {
    for field in [
        "grant_type",
        "requested_scope",
        "scope_reported",
        "target",
        "client_id",
    ] {
        let mut bad = value();
        bad.as_object_mut().unwrap().remove(field);
        assert!(
            serde_json::from_value::<AuthorizationContext>(bad).is_err(),
            "{field}"
        );
    }
    for (field, replacement) in [
        ("grant_type", json!("service_account")),
        ("scope_reported", Value::Null),
        ("target", Value::Null),
        ("access_token", json!("private-token")),
        ("issuer", json!("https://evil.test")),
    ] {
        let mut bad = value();
        bad[field] = replacement;
        assert!(
            serde_json::from_value::<AuthorizationContext>(bad).is_err(),
            "{field}"
        );
    }
    let duplicate =
        serde_json::to_string(&value())
            .unwrap()
            .replacen("{", "{\"client_id\":\"other\",", 1);
    assert!(serde_json::from_str::<AuthorizationContext>(&duplicate).is_err());
}

#[test]
fn vertex_oauth_context_expired_registration_is_readable_but_cannot_prepare_network() {
    let mut expired = value();
    expired["refresh_expires_at"] = json!("2000-01-01T00:00:00Z");
    let ctx = context(expired);
    assert_eq!(ctx.validate(), Ok(()));
    assert_eq!(
        ctx.refresh_grant("synthetic-refresh").unwrap_err(),
        OAuthError::Expired
    );
    assert!(ctx.refresh_grant("bad refresh").is_err());
}

#[test]
fn vertex_oauth_context_new_consent_cannot_reuse_an_access_only_reply() {
    let reply =
        json!({"access_token":"synthetic-access", "token_type":"Bearer", "expires_in":3600});
    let token = TokenMaterial::parse(
        &serde_json::to_vec(&reply).unwrap(),
        false,
        OffsetDateTime::now_utc(),
    )
    .unwrap();
    let result = AuthorizationContext::from_authorization(
        OAuthClient::new("explicit-client", None).unwrap(),
        VertexTarget::new("explicit-project", "global", None).unwrap(),
        &token,
    );
    assert_eq!(result.unwrap_err(), OAuthError::MissingRefreshToken);
}
