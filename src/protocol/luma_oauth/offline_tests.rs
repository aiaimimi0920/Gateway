//! Offline material is accepted only after explicit consent; legacy serialization stays unchanged.
use super::{AuthorizationContext, OAuthError, TokenMaterial};
use time::OffsetDateTime;

fn parse(bytes: &[u8]) -> Result<TokenMaterial, OAuthError> {
    TokenMaterial::parse_offline(
        bytes,
        OffsetDateTime::now_utc(),
        "registered-client".into(),
        "http://127.0.0.1:54421/callback".into(),
    )
}

#[test]
fn luma_offline_material_requires_valid_refresh_and_never_serializes_it() {
    for tail in [
        "",
        ",\"refresh_token\":null",
        ",\"refresh_token\":\"\"",
        ",\"refresh_token\":12",
        ",\"refresh_token\":\"bad\\nvalue\"",
        ",\"refresh_token\":\"a\",\"refresh_token\":\"b\"",
    ] {
        let raw =
            format!("{{\"access_token\":\"synthetic-access\",\"token_type\":\"Bearer\"{tail}}}");
        assert_eq!(
            parse(raw.as_bytes()).unwrap_err(),
            OAuthError::InvalidResponse
        );
    }
    let raw = br#"{"access_token":"synthetic-access","token_type":"Bearer","refresh_token":"synthetic-refresh"}"#;
    let material = parse(raw).unwrap();
    assert_eq!(material.refresh_token(), Some("synthetic-refresh"));
    let context = AuthorizationContext::from_material(&material).unwrap();
    assert!(context.offline_access());
    let encoded = serde_json::to_string(&context).unwrap();
    assert!(!encoded.contains("synthetic-"));
    assert!(!format!("{material:?}").contains("synthetic-"));
    let restored: AuthorizationContext = serde_json::from_str(&encoded).unwrap();
    assert_eq!(restored, context);
    let access_only = restored.restore_material("synthetic-access").unwrap();
    assert!(access_only.offline_access());
    assert_eq!(access_only.refresh_token(), None);
    assert_eq!(
        AuthorizationContext::from_material(&access_only).unwrap(),
        context
    );
    let legacy = TokenMaterial::parse(
        raw,
        OffsetDateTime::now_utc(),
        "registered-client".into(),
        "http://127.0.0.1:54421/callback".into(),
    )
    .unwrap();
    assert_eq!(legacy.refresh_token(), None);
    let mut encoded =
        serde_json::to_value(AuthorizationContext::from_material(&legacy).unwrap()).unwrap();
    assert!(encoded.get("offline_access").is_none());
    encoded["offline_access"] = serde_json::json!(false);
    let legacy: AuthorizationContext = serde_json::from_value(encoded).unwrap();
    assert!(!legacy.offline_access());
    assert!(super::RefreshGrant::new(legacy.clone(), "synthetic-refresh".into()).is_err());
    assert!(serde_json::to_value(legacy)
        .unwrap()
        .get("offline_access")
        .is_none());
}

#[test]
fn luma_offline_material_rejects_oversized_and_non_ascii_refresh() {
    for refresh in ["x".repeat(32769), "秘密".into()] {
        let raw = serde_json::to_vec(&serde_json::json!({"access_token":"synthetic-access",
            "token_type":"Bearer","refresh_token":refresh}))
        .unwrap();
        assert_eq!(parse(&raw).unwrap_err(), OAuthError::InvalidResponse);
    }
}
