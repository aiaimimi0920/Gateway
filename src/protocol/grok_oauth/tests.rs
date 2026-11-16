use super::*;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
mod adversarial;
mod grants;
mod identity;
mod identity_support;
mod identity_wire;
#[cfg(feature = "family-openai-compatible-official-api")]
mod lookups;
#[cfg(feature = "family-openai-compatible-official-api")]
mod product;
#[cfg(feature = "family-openai-compatible-official-api")]
mod product_mutations;
mod raw_transport;
mod wire;

fn client() -> OAuthClient {
    OAuthClient::new(
        "gateway-test-client",
        &["openid", "offline_access", "grok-cli:access"],
    )
    .unwrap()
}

#[test]
fn authorize_uses_separate_nonce_state_pkce_and_fixed_product() {
    let client = client().with_principal(Principal::new("Team", "team-123").unwrap());
    let auth = Authorization::new(&client, "http://127.0.0.1:45678/callback").unwrap();
    let url = url::Url::parse(&auth.authorize_url()).unwrap();
    let query: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();
    assert_eq!(url.origin().ascii_serialization(), ISSUER);
    assert_eq!(url.path(), "/oauth2/authorize");
    assert_eq!(query["client_id"], "gateway-test-client");
    assert_eq!(query["code_challenge_method"], "S256");
    assert_eq!(query["principal_type"], "Team");
    assert_eq!(query["principal_id"], "team-123");
    assert_eq!(query["referrer"], "neuro-gateway");
    assert_ne!(query["state"], query["nonce"]);
    assert_eq!(
        query["code_challenge"],
        URL_SAFE_NO_PAD.encode(Sha256::digest(auth.verifier.as_bytes()))
    );
    assert_eq!(query["code_challenge"].len(), 43);
    let debug = format!("{auth:?}");
    for secret in [&auth.verifier, &auth.state, &auth.nonce] {
        assert!(!debug.contains(secret));
    }
    assert_eq!(
        auth.callback_code(&format!(
            "{}?code=synthetic-code&state={}",
            auth.redirect_uri, auth.state
        ))
        .unwrap(),
        "synthetic-code"
    );
}

#[test]
fn callback_rejects_wrong_origin_duplicates_controls_and_wrong_state() {
    let auth = Authorization::new(&client(), "http://127.0.0.1:45678/callback").unwrap();
    for value in [
        format!(
            "http://127.0.0.1:45679/callback?code=x&state={}",
            auth.state
        ),
        format!(
            "http://localhost:45678/callback?code=x&state={}",
            auth.state
        ),
        format!("{}?code=x&code=y&state={}", auth.redirect_uri, auth.state),
        format!(
            "{}?code=x&state={}&state={}",
            auth.redirect_uri, auth.state, auth.state
        ),
        format!("{}?code=x&state=wrong", auth.redirect_uri),
        format!("{}?code=x&state={}#fragment", auth.redirect_uri, auth.state),
        format!(
            "{}?code=x%0Aprivate&state={}",
            auth.redirect_uri, auth.state
        ),
        format!(
            "{}?error=access_denied&code=x&state={}",
            auth.redirect_uri, auth.state
        ),
    ] {
        assert!(auth.callback_code(&value).is_err());
    }
    let error = auth
        .callback_code(&format!(
            "{}?error=access_denied&error_description=private-detail&state={}",
            auth.redirect_uri, auth.state
        ))
        .unwrap_err();
    assert_eq!(error, OAuthError::AccessDenied);
    assert!(!format!("{error:?} {error}").contains("private-detail"));
}

#[test]
fn explicit_client_and_loopback_configuration_are_bounded() {
    for value in ["", "bad client", "private\nvalue"] {
        assert!(OAuthClient::new(value, &["openid"]).is_err());
    }
    assert!(OAuthClient::new("client", &[]).is_err());
    assert!(OAuthClient::new("client", &["open id"]).is_err());
    assert!(OAuthClient::new("client", &["openid", "openid"]).is_err());
    for value in [
        "https://example.test/callback",
        "http://127.0.0.1/callback",
        "http://127.0.0.1:1234/other",
        "http://user@127.0.0.1:1234/callback",
        "http://127.0.0.1:1234/callback?x=1",
    ] {
        assert!(Authorization::new(&client(), value).is_err());
    }
    assert!(Principal::new("Team", "").is_err());
    assert!(Principal::new("Team", "bad\nprincipal").is_err());
    assert!(Authorization::new(&client(), "http://[::1]:1234/callback").is_ok());
}

#[test]
fn token_material_is_unverified_bounded_and_never_debug_printed() {
    let tokens = UnverifiedTokens::parse(json!({"access_token":"private-access","refresh_token":"private-refresh","id_token":"private-id","expires_in":60,"token_type":"Bearer"})).unwrap();
    assert_eq!(tokens.access_token, "private-access");
    assert_eq!(tokens.refresh_token.as_deref(), Some("private-refresh"));
    assert!(!format!("{tokens:?}").contains("private-"));
    for body in [
        json!({"access_token":""}),
        json!({"access_token":"x\ny"}),
        json!({"access_token":"access","expires_in":0}),
        json!({"access_token":"access","expires_in":-1}),
        json!({"access_token":"access","expires_in":u64::MAX}),
        json!({"access_token":"access","refresh_token":""}),
        json!({"access_token":"access","token_type":"MAC"}),
        json!({"access_token":"a".repeat(32769)}),
    ] {
        assert_eq!(
            UnverifiedTokens::parse(body).unwrap_err(),
            OAuthError::InvalidResponse
        );
    }
    assert!(UnverifiedTokens::parse(json!({"access_token":"opaque-access"})).is_ok());
}

#[test]
fn device_material_has_fixed_link_audience_and_no_secret_debug() {
    let body = json!({"device_code":"private-device","user_code":"ABCD-EFGH","verification_uri":"https://accounts.x.ai/device","verification_uri_complete":"https://accounts.x.ai/device?user_code=ABCD-EFGH","expires_in":300,"interval":1});
    let device = DeviceChallenge::parse(body.clone(), &client()).unwrap();
    assert_eq!(device.user_code, "ABCD-EFGH");
    assert!(!format!("{device:?}").contains("private-device"));
    for (field, value) in [
        ("verification_uri", json!("https://example.test/device")),
        (
            "verification_uri",
            json!("https://user@accounts.x.ai/device"),
        ),
        (
            "verification_uri",
            json!("https://accounts.x.ai:444/device"),
        ),
        (
            "verification_uri_complete",
            json!("https://example.test/private-device"),
        ),
        ("user_code", json!("ABCD\nsecret")),
        ("device_code", json!("")),
        ("expires_in", json!(0)),
        ("interval", json!(0)),
    ] {
        let mut bad = body.clone();
        bad[field] = value;
        assert_eq!(
            DeviceChallenge::parse(bad, &client()).unwrap_err(),
            OAuthError::InvalidResponse
        );
    }
    let mut long = body;
    long["expires_in"] = json!(86400);
    assert!(
        DeviceChallenge::parse(long, &client())
            .unwrap()
            .remaining()
            .as_secs()
            <= 600
    );
}
