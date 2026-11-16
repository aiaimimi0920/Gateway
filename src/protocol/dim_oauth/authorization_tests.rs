use super::*;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

pub(super) fn document(site: Site) -> Value {
    let issuer = site.issuer();
    json!({"issuer":issuer,"authorization_endpoint":format!("{issuer}/oauth/authorize"),
        "token_endpoint":format!("{issuer}/oauth/token"),"userinfo_endpoint":format!("{issuer}/oauth/userinfo"),
        "jwks_uri":format!("{issuer}/oauth/jwks.json"),"response_types_supported":["code"],
        "code_challenge_methods_supported":["S256"],"token_endpoint_auth_methods_supported":["none"],
        "id_token_signing_alg_values_supported":["RS256"]})
}
fn attempt() -> Authorization {
    let client = OAuthClient::new(
        Site::China,
        "permitted-gateway",
        &["openid", "profile", "email"],
    )
    .unwrap();
    let metadata = Metadata::parse(
        &client,
        &serde_json::to_vec(&document(Site::China)).unwrap(),
    )
    .unwrap();
    Authorization::new(metadata, "http://127.0.0.1:54322/oauth/callback").unwrap()
}

#[test]
fn dim_discovery_rejects_cross_site_userinfo_jwks_and_confused_public_client() {
    let client = OAuthClient::new(Site::China, "permitted-gateway", &["openid"]).unwrap();
    for (key, value) in [
        ("issuer", json!(Site::Global.issuer())),
        ("token_endpoint", json!("https://dimagent.com/oauth/token")),
        (
            "userinfo_endpoint",
            json!("https://dimagent.cn.evil.invalid/oauth/userinfo"),
        ),
        (
            "jwks_uri",
            json!("https://dimagent.cn/oauth/jwks.json?key=private"),
        ),
        (
            "authorization_endpoint",
            json!("https://user:pass@dimagent.cn/oauth/authorize"),
        ),
        ("code_challenge_methods_supported", json!(["plain"])),
        (
            "token_endpoint_auth_methods_supported",
            json!(["client_secret_basic"]),
        ),
        ("id_token_signing_alg_values_supported", json!(["none"])),
    ] {
        let mut invalid = document(Site::China);
        invalid[key] = value;
        assert!(Metadata::parse(&client, &serde_json::to_vec(&invalid).unwrap()).is_err());
    }
    assert!(Metadata::parse(
        &client,
        &serde_json::to_vec(&document(Site::China)).unwrap()
    )
    .is_ok());
}

#[test]
fn dim_authorization_keeps_state_nonce_pkce_and_redirect_independent_and_redacted() {
    let auth = attempt();
    let url = url::Url::parse(&auth.authorize_url()).unwrap();
    let fields: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();
    assert_eq!(url.origin().ascii_serialization(), Site::China.issuer());
    assert_eq!(url.path(), "/oauth/authorize");
    assert_eq!(fields["scope"], "openid profile email");
    assert_eq!(fields["code_challenge_method"], "S256");
    assert_ne!(fields["state"], fields["nonce"]);
    assert_ne!(auth.verifier, fields["state"]);
    assert_eq!(
        fields["code_challenge"],
        URL_SAFE_NO_PAD.encode(Sha256::digest(auth.verifier.as_bytes()))
    );
    assert!(!format!("{auth:?}").contains(&fields["state"]));
    assert!(!format!("{auth:?}").contains(&auth.verifier));
    assert_eq!(auth.nonce(), fields["nonce"]);
    let another = attempt();
    assert_ne!(another.state, auth.state);
}

#[test]
fn dim_callback_rejects_duplicates_mixup_and_wrong_origin_before_code_consumption() {
    let auth = attempt();
    let good = format!(
        "{}?state={}&code=synthetic-code",
        auth.redirect_uri(),
        auth.state
    );
    for callback in [
        good.replace("127.0.0.1", "localhost"),
        good.replace("54322", "54323"),
        good.replace("/oauth/callback", "/other"),
        format!("{good}&state={}", auth.state),
        format!("{good}&code=second"),
        format!("{good}&iss=https%3A%2F%2Fdimagent.com"),
        format!("{good}&error=access_denied"),
        format!("{good}#fragment"),
    ] {
        assert!(auth.callback_code(&callback).is_err());
    }
    assert_eq!(auth.callback_code(&good).unwrap(), "synthetic-code");
    let denied = format!(
        "{}?state={}&error=access_denied",
        auth.redirect_uri(),
        auth.state
    );
    assert!(matches!(
        auth.callback_code(&denied),
        Err(OAuthError::AccessDenied)
    ));
    let grant = auth.into_code_grant(&good).unwrap();
    assert!(!format!("{grant:?}").contains("synthetic-code"));
    assert_eq!(grant.code, "synthetic-code");
    assert!(!grant.authorization.remaining().is_zero());
}

#[test]
fn dim_redirect_requires_explicit_registered_ipv4_loopback_and_local_expiry_is_not_extended() {
    for redirect in [
        "https://127.0.0.1:54322/oauth/callback",
        "http://localhost:54322/oauth/callback",
        "http://127.0.0.1/oauth/callback",
        "http://127.0.0.1:0/oauth/callback",
        "http://127.0.0.1:54322/callback",
        "http://127.0.0.1:54322/oauth/callback?x=1",
        "http://127.1:54322/oauth/callback",
        "http://127.0.0.1:054322/oauth/callback",
        "HTTP://127.0.0.1:54322/oauth/callback",
    ] {
        assert!(Authorization::callback_port(redirect).is_err());
        assert!(Authorization::new(attempt().metadata, redirect).is_err());
    }
    assert_eq!(
        Authorization::callback_port("http://127.0.0.1:54322/oauth/callback").unwrap(),
        54322
    );
    let mut auth = attempt();
    let callback = format!(
        "{}?state={}&code=synthetic",
        auth.redirect_uri(),
        auth.state
    );
    auth.deadline = tokio::time::Instant::now();
    assert!(matches!(
        auth.callback_code(&callback),
        Err(OAuthError::Expired)
    ));
}
