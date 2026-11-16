//! Restored registrations retain absolute expiry and bounded, secret-safe metadata.
use super::*;
use serde_json::{json, Value};

fn value() -> Value {
    json!({"auth_method":"builder_id","directory_region":"us-east-1",
        "start_url":"https://view.awsapps.com/start","client_id":"synthetic-client",
        "client_secret":"synthetic-secret","client_secret_expires_at":1893456000i64,
        "scopes":["codewhisperer:conversations"],"machine_id":"a".repeat(64)})
}
fn context(value: Value) -> AuthorizationContext {
    serde_json::from_value(value).unwrap()
}

#[tokio::test]
async fn restored_context_preserves_registration_expiry_directory_scopes_and_machine() {
    let expected = value();
    let original = context(expected.clone());
    let registration = original.registration().unwrap();
    assert_eq!(registration.expires_at().unix_timestamp(), 1893456000);
    assert_eq!(registration.directory(), &Directory::builder_id());
    assert_eq!(registration.scopes(), &[Scope::Conversations]);
    let next =
        AuthorizationContext::from_registration(&registration, original.machine_id()).unwrap();
    assert_eq!(serde_json::to_value(next).unwrap(), expected);
    assert!(!format!("{original:?} {registration:?}").contains("synthetic"));
}

#[tokio::test]
async fn expired_registration_remains_readable_but_cannot_regain_lifetime_on_restore() {
    let mut old = value();
    old["client_secret_expires_at"] = json!(1);
    let original = context(old);
    original.validate().unwrap();
    for _ in 0..2 {
        let restored: AuthorizationContext =
            serde_json::from_str(&serde_json::to_string(&original).unwrap()).unwrap();
        assert_eq!(
            restored.registration().unwrap_err(),
            OAuthError::RegistrationExpired
        );
    }
}

#[tokio::test]
async fn redacted_metadata_is_serializable_but_never_a_refresh_capability() {
    let mut original = context(value());
    assert_eq!(
        original.take_client_secret().as_deref(),
        Some("synthetic-secret")
    );
    original.validate_metadata().unwrap();
    assert!(original.validate().is_err());
    assert!(original.registration().is_err());
    let serialized = serde_json::to_value(&original).unwrap();
    assert!(serialized.get("client_secret").is_none());
    assert_eq!(context(serialized), original);
}

#[test]
fn context_rejects_invalid_directory_scopes_secret_and_device_identity() {
    for (key, invalid) in [
        ("auth_method", json!("social")),
        ("directory_region", json!("us-west-2")),
        ("start_url", json!("https://foreign.invalid/start")),
        ("client_id", json!("bad\nid")),
        ("client_secret", json!(" ")),
        ("client_secret_expires_at", json!(0)),
        ("client_secret_expires_at", json!(i64::MAX)),
        ("scopes", json!([])),
        (
            "scopes",
            json!(["codewhisperer:conversations", "codewhisperer:conversations"]),
        ),
        ("scopes", json!(["sso:account:access"])),
        ("machine_id", json!("A".repeat(64))),
        ("machine_id", json!("a".repeat(63))),
        ("access_token", json!("must-not-be-hidden")),
    ] {
        let mut next = value();
        next[key] = invalid;
        let result = serde_json::from_value::<AuthorizationContext>(next)
            .ok()
            .and_then(|context| context.validate().ok());
        assert!(result.is_none(), "accepted invalid {key}");
    }
    let duplicate = serde_json::to_string(&value()).unwrap().replacen(
        "\"client_id\":",
        "\"client_id\":\"duplicate\",\"client_id\":",
        1,
    );
    assert!(serde_json::from_str::<AuthorizationContext>(&duplicate).is_err());
}

#[tokio::test]
async fn identity_center_directory_stays_independent_of_business_region() {
    let mut next = value();
    next["auth_method"] = json!("identity_center");
    next["directory_region"] = json!("eu-west-1");
    next["start_url"] = json!("https://synthetic.awsapps.com/start");
    let context = context(next);
    assert_eq!(
        context.token_endpoint(),
        "https://oidc.eu-west-1.amazonaws.com/token"
    );
    assert_eq!(
        context.registration().unwrap().directory().region(),
        "eu-west-1"
    );
}
