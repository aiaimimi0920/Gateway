use super::*;
use serde_json::{json, Value};
use time::{Duration, OffsetDateTime};

pub(super) fn tokens() -> Value {
    json!({"access_token":"access-private", "refresh_token":"refresh-private", "token_type":"Bearer",
        "expires_in":3600, "scope":CLOUD_PLATFORM_SCOPE, "refresh_token_expires_in":7200})
}
fn parse(value: Value, require_refresh: bool) -> Result<TokenMaterial, OAuthError> {
    TokenMaterial::parse(
        &serde_json::to_vec(&value).unwrap(),
        require_refresh,
        OffsetDateTime::now_utc(),
    )
}

#[test]
fn vertex_oauth_material_preserves_absolute_access_and_time_based_refresh_expiry() {
    let started = OffsetDateTime::now_utc() - Duration::seconds(2);
    let token =
        TokenMaterial::parse(&serde_json::to_vec(&tokens()).unwrap(), true, started).unwrap();
    assert_eq!(token.access_token(), "access-private");
    assert_eq!(token.refresh_token(), Some("refresh-private"));
    assert_eq!(token.expires_at(), started + Duration::seconds(3600));
    assert_eq!(token.expires_in(), 3600);
    assert_eq!(
        token.refresh_expires_at(),
        Some(started + Duration::seconds(7200))
    );
    assert_eq!(token.reported_scope(), Some(CLOUD_PLATFORM_SCOPE));
}

#[test]
fn vertex_oauth_material_requires_fresh_refresh_for_new_consent_not_an_old_grant() {
    let mut value = tokens();
    value.as_object_mut().unwrap().remove("refresh_token");
    assert_eq!(
        parse(value.clone(), true).unwrap_err(),
        OAuthError::MissingRefreshToken
    );
    assert!(parse(value, false).unwrap().refresh_token().is_none());
    for refresh in [json!(""), json!("a b"), json!({})] {
        let mut value = tokens();
        value["refresh_token"] = refresh;
        assert!(parse(value, false).is_err());
    }
}

#[test]
fn vertex_oauth_material_validates_known_types_tokens_and_duration_overflow() {
    for (field, invalid) in [
        ("access_token", json!("")),
        ("access_token", json!("private\r\nvalue")),
        ("access_token", json!("x".repeat(32_769))),
        ("expires_in", json!(0)),
        ("expires_in", json!("3600")),
        ("expires_in", json!(u64::MAX)),
        ("refresh_token_expires_in", json!(0)),
        ("refresh_token_expires_in", json!(u64::MAX)),
        ("token_type", json!("DPoP")),
        ("token_type", json!(" Bearer")),
        ("error", json!("private-upstream-error")),
    ] {
        let mut value = tokens();
        value[field] = invalid;
        assert!(parse(value, true).is_err(), "field {field}");
    }
    for field in ["access_token", "expires_in", "token_type"] {
        let mut value = tokens();
        value.as_object_mut().unwrap().remove(field);
        assert!(parse(value, true).is_err());
    }
}

#[test]
fn vertex_oauth_material_scope_is_a_report_not_an_unverified_user_identity() {
    for scope in [
        "",
        "openid",
        "https://www.googleapis.com/auth/drive",
        "https://www.googleapis.com/auth/cloud-platform openid",
    ] {
        let mut value = tokens();
        value["scope"] = json!(scope);
        assert_eq!(
            parse(value, true).unwrap_err(),
            OAuthError::InsufficientScope
        );
    }
    let mut value = tokens();
    value.as_object_mut().unwrap().remove("scope");
    value["id_token"] = json!("unverified-private-identity");
    value["unknown_future_field"] = json!({"opaque":true});
    let token = parse(value, true).unwrap();
    assert_eq!(token.reported_scope(), None);
    let debug = format!("{token:?}");
    for hidden in [
        "access-private",
        "refresh-private",
        "unverified-private-identity",
    ] {
        assert!(!debug.contains(hidden));
    }
}

#[test]
fn vertex_oauth_material_rejects_duplicate_known_fields_and_oversized_reply() {
    let bytes = br#"{"access_token":"first","access_token":"second","expires_in":3600,"token_type":"Bearer","refresh_token":"r"}"#;
    assert_eq!(
        TokenMaterial::parse(bytes, true, OffsetDateTime::now_utc()).unwrap_err(),
        OAuthError::InvalidResponse
    );
    assert_eq!(
        TokenMaterial::parse(&vec![b' '; 65_537], true, OffsetDateTime::now_utc()).unwrap_err(),
        OAuthError::ResponseTooLarge
    );
}

#[test]
fn vertex_oauth_material_rejects_null_known_fields_but_accepts_unknown_evolution() {
    for field in [
        "access_token",
        "refresh_token",
        "expires_in",
        "token_type",
        "scope",
        "refresh_token_expires_in",
        "error",
    ] {
        let mut value = tokens();
        value[field] = Value::Null;
        assert_eq!(
            parse(value, false).unwrap_err(),
            OAuthError::InvalidResponse,
            "{field}"
        );
    }
    let mut value = tokens();
    value["token_type"] = json!("bearer");
    value["future-field"] = json!(null);
    assert!(parse(value, true).is_ok());
    for field in [
        "refresh_token",
        "scope",
        "token_type",
        "expires_in",
        "refresh_token_expires_in",
    ] {
        let original = tokens().to_string();
        let duplicate = format!(
            "{},\"{field}\":{}}}",
            &original[..original.len() - 1],
            tokens()[field]
        );
        assert_eq!(
            TokenMaterial::parse(duplicate.as_bytes(), true, OffsetDateTime::now_utc())
                .unwrap_err(),
            OAuthError::InvalidResponse,
            "{field}"
        );
    }
}

#[test]
fn vertex_oauth_material_does_not_reset_elapsed_lifetime_at_receive_time() {
    let bytes = serde_json::to_vec(&tokens()).unwrap();
    assert_eq!(
        TokenMaterial::parse(
            &bytes,
            true,
            OffsetDateTime::now_utc() - Duration::seconds(4000)
        )
        .unwrap_err(),
        OAuthError::Expired
    );
    assert_eq!(
        TokenMaterial::parse(&bytes, true, time::PrimitiveDateTime::MAX.assume_utc()).unwrap_err(),
        OAuthError::InvalidResponse
    );
}
