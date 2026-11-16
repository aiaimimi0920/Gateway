//! Public CLI contract, not OAuth grants or a proof of the user's quota/identity.
use super::*;
use serde_json::{json, Value};
use std::time::Duration;
use tokio::time::Instant;

pub(super) fn code() -> Value {
    json!({"loginUrl":"https://freebuff.com/login?code=synthetic-browser-code",
        "fingerprintHash":"synthetic-hash+/=", "expiresAt":"opaque +/= expiry:原样"})
}
fn parse(value: Value, started: Instant) -> Result<LoginChallenge, LoginError> {
    LoginChallenge::parse(
        "enhanced-synthetic-device",
        Reply {
            status: 200,
            bytes: value.to_string().into_bytes(),
        },
        started,
    )
}

#[tokio::test]
async fn opaque_expiry_and_official_relative_lifetime_are_not_wall_clock_timestamps() {
    let challenge = parse(code(), Instant::now()).unwrap();
    assert_eq!(challenge.expires_at, "opaque +/= expiry:原样");
    assert!(challenge.remaining() <= Duration::from_secs(3600));
    assert!(challenge.remaining() > Duration::from_secs(3590));
    assert_eq!(challenge.interval(), Duration::from_secs(5));
    assert_eq!(
        challenge.login_url(),
        "https://freebuff.com/login?code=synthetic-browser-code"
    );
    let mut short = code();
    short["expiresInMs"] = json!(2500);
    let challenge = parse(short, Instant::now() - Duration::from_secs(1)).unwrap();
    assert!(challenge.remaining() <= Duration::from_millis(1500));
    let mut long = code();
    long["expiresInMs"] = json!(u64::MAX);
    assert!(parse(long, Instant::now()).unwrap().remaining() <= Duration::from_secs(3600));
}

#[tokio::test]
async fn returned_browser_url_cannot_choose_another_origin_or_credential_surface() {
    for url in [
        "http://freebuff.com/login",
        "https://freebuff.com.evil.invalid/login",
        "https://codebuff.com/login",
        "https://freebuff.com@evil.invalid/",
        "https://user@freebuff.com/",
        "https://freebuff.com:8443/",
        "https://freebuff.com/login#private",
        "file:///C:/private",
        "/login",
    ] {
        let mut value = code();
        value["loginUrl"] = json!(url);
        assert_eq!(
            parse(value, Instant::now()).unwrap_err(),
            LoginError::InvalidResponse,
            "{url}"
        );
    }
}

#[tokio::test]
async fn missing_conflicting_and_expired_code_fields_do_not_create_a_capability() {
    for (field, value) in [
        ("fingerprintHash", json!("")),
        ("expiresAt", json!("")),
        ("expiresAt", json!("private\nmaterial")),
        ("expiresInMs", json!(0)),
        ("expiresInMs", json!(-1)),
        ("expiresInMs", json!(1.5)),
        ("error", json!("private")),
    ] {
        let mut body = code();
        body[field] = value;
        assert!(parse(body, Instant::now()).is_err(), "{field}");
    }
    let mut short = code();
    short["expiresInMs"] = json!(1);
    assert_eq!(
        parse(short, Instant::now() - Duration::from_secs(1)).unwrap_err(),
        LoginError::Expired
    );
    let mut reply = code().to_string();
    reply.pop();
    reply.push_str(",\"fingerprintHash\":\"other\"}");
    assert_eq!(
        LoginChallenge::parse(
            "device",
            Reply {
                status: 200,
                bytes: reply.into_bytes()
            },
            Instant::now()
        )
        .unwrap_err(),
        LoginError::InvalidResponse
    );
}

#[test]
fn credentials_bind_both_fingerprints_and_do_not_infer_token_expiry_or_verified_identity() {
    let user = json!({"id":"user-1", "email":"synthetic@example.invalid", "name":null,
        "authToken":"synthetic-business-token", "fingerprintId":"device", "fingerprintHash":"hash"});
    let accept =
        |value| Credentials::accept(serde_json::from_value(value).unwrap(), "device", "hash");
    let material = accept(user.clone()).unwrap();
    assert_eq!(material.auth_token(), "synthetic-business-token");
    assert_eq!(material.fingerprint_id(), "device");
    assert_eq!(material.fingerprint_hash(), "hash");
    assert_eq!(material.user_id(), "user-1");
    for (field, value) in [
        ("fingerprintId", "other"),
        ("fingerprintHash", "other"),
        ("authToken", ""),
        ("authToken", "with space"),
        ("id", ""),
    ] {
        let mut changed = user.clone();
        changed[field] = json!(value);
        assert_eq!(accept(changed).unwrap_err(), LoginError::InvalidResponse);
    }
    let text = format!("{material:?}");
    for secret in ["synthetic", "device", "hash", "user-1"] {
        assert!(!text.contains(secret));
    }
}

#[tokio::test]
async fn cancellation_expiry_and_debug_do_not_expose_or_reopen_a_challenge() {
    let mut challenge = parse(code(), Instant::now()).unwrap();
    let text = format!("{challenge:?}");
    for secret in ["synthetic", "opaque", "enhanced", "https"] {
        assert!(!text.contains(secret));
    }
    challenge.cancel();
    assert_eq!(
        challenge
            .poll(&LoginTransport::new().unwrap())
            .await
            .unwrap_err(),
        LoginError::Cancelled
    );
    let mut challenge = parse(code(), Instant::now()).unwrap();
    challenge.deadline = Instant::now();
    assert_eq!(
        challenge
            .poll(&LoginTransport::new().unwrap())
            .await
            .unwrap_err(),
        LoginError::Expired
    );
}

#[cfg(not(feature = "line-freebuff-web-reverse-api"))]
#[tokio::test]
async fn disabled_login_cannot_send_requests_or_create_an_enabled_capability() {
    assert_eq!(
        LoginChallenge::start(&LoginTransport::new().unwrap(), "explicit-device")
            .await
            .unwrap_err(),
        LoginError::FeatureDisabled
    );
}
