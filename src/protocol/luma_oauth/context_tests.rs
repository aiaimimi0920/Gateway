//! Durable metadata cannot move a bearer to another audience or renew its lifetime.
use super::{AuthorizationContext, OAuthError, TokenMaterial, ISSUER, RESOURCE, TOKEN_ENDPOINT};
use serde_json::json;
use time::OffsetDateTime;

fn material(expiry: bool) -> TokenMaterial {
    let mut reply = json!({"access_token":"synthetic-secret","token_type":"bearer"});
    if expiry {
        reply["expires_in"] = json!(3600);
    }
    TokenMaterial::parse(
        &serde_json::to_vec(&reply).unwrap(),
        OffsetDateTime::now_utc(),
        "registered-client".into(),
        "http://127.0.0.1:54421/callback".into(),
    )
    .unwrap()
}

#[test]
fn luma_oauth_context_roundtrip_preserves_binding_secret_and_absolute_expiry() {
    for expiry in [false, true] {
        let source = material(expiry);
        let context = AuthorizationContext::from_material(&source).unwrap();
        let encoded = serde_json::to_string(&context).unwrap();
        assert!(!encoded.contains(source.access_token()));
        assert!(!format!("{context:?}").contains("registered-client"));
        let decoded: AuthorizationContext = serde_json::from_str(&encoded).unwrap();
        assert_eq!(context, decoded);
        let restored = decoded.restore_material(source.access_token()).unwrap();
        assert_eq!(restored.access_token(), source.access_token());
        assert_eq!(restored.client_id(), source.client_id());
        assert_eq!(restored.redirect_uri(), source.redirect_uri());
        assert_eq!(restored.expires_at(), source.expires_at());
        assert_eq!(serde_json::to_value(&context).unwrap()["issuer"], ISSUER);
        assert_eq!(
            serde_json::to_value(&context).unwrap()["resource"],
            RESOURCE
        );
        assert_eq!(
            serde_json::to_value(&context).unwrap()["token_endpoint"],
            TOKEN_ENDPOINT
        );
    }
}

#[test]
fn luma_oauth_context_rejects_untrusted_authority_metadata_and_unknown_fields() {
    let base = serde_json::to_value(AuthorizationContext::from_material(&material(true)).unwrap())
        .unwrap();
    for (key, value) in [
        ("issuer", json!("https://other.invalid")),
        ("resource", json!("https://agents.lumalabs.ai/v1")),
        ("token_endpoint", json!("https://other.invalid/token")),
        ("client_id", json!("")),
        ("client_id", json!("bad\nclient")),
        ("redirect_uri", json!("https://other.invalid/callback")),
        (
            "redirect_uri",
            json!("http://127.0.0.1:54421/callback?code=private"),
        ),
        ("expires_at", json!("not-a-time")),
        ("access_token", json!("private")),
        ("subject", json!("unverified")),
    ] {
        let mut changed = base.clone();
        changed[key] = value;
        assert!(
            serde_json::from_value::<AuthorizationContext>(changed).is_err(),
            "{key}"
        );
    }
    let raw = serde_json::to_string(&base).unwrap();
    let duplicate = raw.replacen('{', "{\"client_id\":\"duplicate\",", 1);
    assert!(serde_json::from_str::<AuthorizationContext>(&duplicate).is_err());
}

#[test]
fn luma_oauth_context_expired_storage_is_readable_but_never_revived() {
    let mut value =
        serde_json::to_value(AuthorizationContext::from_material(&material(true)).unwrap())
            .unwrap();
    value["expires_at"] = json!("2020-01-01T00:00:00Z");
    let context: AuthorizationContext = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(
        context.restore_material("synthetic-secret").unwrap_err(),
        OAuthError::Expired
    );
    assert_eq!(serde_json::to_value(&context).unwrap(), value);
    let context = AuthorizationContext::from_material(&material(false)).unwrap();
    for access in ["", "bad\r\nheader", "secret with space"] {
        assert_eq!(
            context.restore_material(access).unwrap_err(),
            OAuthError::InvalidInput
        );
    }
    assert!(context.restore_material(&"x".repeat(32769)).is_err());
    assert_eq!(
        serde_json::to_value(context).unwrap().get("expires_at"),
        None
    );
}
