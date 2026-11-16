use super::*;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

fn auth(site: LoginSite) -> AuthorizationAttempt {
    AuthorizationAttempt::new(OAuthClient::new("synthetic-client").unwrap(), site, 34567).unwrap()
}
pub(super) fn callback(auth: &AuthorizationAttempt) -> String {
    let url = url::Url::parse(&auth.authorize_url()).unwrap();
    let state = url
        .query_pairs()
        .find(|(key, _)| key == "state")
        .unwrap()
        .1
        .into_owned();
    format!("{}?code=synthetic-code&state={state}", auth.redirect_uri())
}

#[test]
fn accio_pkce_login_has_exact_fixed_site_and_explicit_client_parameters() {
    for (site, origin) in [
        (LoginSite::China, "https://www.accio-ai.com"),
        (LoginSite::Global, "https://zh.accio.com"),
    ] {
        let auth = auth(site);
        let url = url::Url::parse(&auth.authorize_url()).unwrap();
        assert_eq!(url.origin().ascii_serialization(), origin);
        assert_eq!(url.path(), "/login");
        let pairs: HashMap<_, _> = url.query_pairs().into_owned().collect();
        assert_eq!(pairs.len(), 7);
        assert_eq!(pairs["return_url"], "http://127.0.0.1:34567/auth/callback");
        assert_eq!(pairs["client_id"], "synthetic-client");
        assert_eq!(pairs["code_challenge_method"], "S256");
        assert_eq!(pairs["language"], "zh");
        assert_eq!(hex::decode(&pairs["state"]).unwrap().len(), 32);
        assert_eq!(
            hex::decode(pairs["ttid"].strip_prefix("ttid_").unwrap())
                .unwrap()
                .len(),
            16
        );
        let callback = callback(&auth);
        let grant = auth.finish(&callback).unwrap();
        assert_eq!(grant.login_site(), site);
        assert_eq!(URL_SAFE_NO_PAD.decode(&grant.verifier).unwrap().len(), 48);
        assert_eq!(
            URL_SAFE_NO_PAD.encode(Sha256::digest(grant.verifier.as_bytes())),
            pairs["code_challenge"]
        );
        assert_eq!(grant.code, "synthetic-code");
    }
}

#[test]
fn accio_authorization_attempts_do_not_share_pkce_state_or_tracking_id() {
    let a = auth(LoginSite::China);
    let b = auth(LoginSite::China);
    let pairs = |auth: &AuthorizationAttempt| -> HashMap<String, String> {
        url::Url::parse(&auth.authorize_url())
            .unwrap()
            .query_pairs()
            .into_owned()
            .collect()
    };
    for key in ["state", "code_challenge", "ttid"] {
        assert_ne!(pairs(&a)[key], pairs(&b)[key]);
    }
}

#[test]
fn accio_callback_rejects_wrong_origin_path_port_fragment_or_url_credentials() {
    for change in [
        "https://127.0.0.1:34567/auth/callback",
        "http://localhost:34567/auth/callback",
        "http://127.0.0.1:34568/auth/callback",
        "http://127.0.0.1:34567/callback",
        "http://attacker.invalid/auth/callback",
        "http://user:pass@127.0.0.1:34567/auth/callback",
    ] {
        let auth = auth(LoginSite::China);
        let callback = callback(&auth).replace(auth.redirect_uri(), change);
        assert_eq!(
            auth.finish(&callback).unwrap_err(),
            OAuthError::InvalidCallback,
            "{change}"
        );
    }
    for suffix in [
        "#code=private",
        "\n",
        "&state=duplicate",
        "&%73tate=duplicate",
        "&code=duplicate",
        "&unknown=private",
    ] {
        let auth = auth(LoginSite::China);
        let callback = format!("{}{suffix}", callback(&auth));
        assert_eq!(
            auth.finish(&callback).unwrap_err(),
            OAuthError::InvalidCallback
        );
    }
}

#[test]
fn accio_callback_requires_exact_state_and_unambiguous_nonempty_code() {
    for query in [
        "code=private",
        "code=private&state=wrong",
        "code=private&state=",
    ] {
        let auth = auth(LoginSite::China);
        let callback = format!("{}?{query}", auth.redirect_uri());
        assert_eq!(
            auth.finish(&callback).unwrap_err(),
            OAuthError::StateMismatch
        );
    }
    for replace in [
        "",
        "code=",
        "code=bad%0Acode",
        "code=bad%20code",
        "error=access_denied&code=private",
        "error_description=private",
    ] {
        let auth = auth(LoginSite::China);
        let callback = callback(&auth).replace("code=synthetic-code", replace);
        assert_eq!(
            auth.finish(&callback).unwrap_err(),
            OAuthError::InvalidCallback
        );
    }
}

#[test]
fn accio_callback_error_is_local_and_expired_attempt_cannot_be_claimed() {
    for (error, expected) in [
        ("access_denied", OAuthError::AccessDenied),
        ("private-upstream-text", OAuthError::Rejected),
    ] {
        let auth = auth(LoginSite::China);
        let callback = callback(&auth).replace(
            "code=synthetic-code",
            &format!("error={error}&error_description=private-token"),
        );
        let actual = auth.finish(&callback).unwrap_err();
        assert_eq!(actual, expected);
        assert!(!actual.to_string().contains("private"));
    }
    let mut auth = auth(LoginSite::China);
    let callback = callback(&auth);
    auth.expire();
    assert_eq!(auth.finish(&callback).unwrap_err(), OAuthError::Expired);
}

#[test]
fn accio_client_and_refresh_input_bounds_do_not_allow_empty_or_control_material() {
    for client in ["", " ", "bad\nclient", "has space", &"a".repeat(513)] {
        assert_eq!(
            OAuthClient::new(client).unwrap_err(),
            OAuthError::InvalidInput
        );
    }
    assert_eq!(
        AuthorizationAttempt::new(OAuthClient::new("explicit").unwrap(), LoginSite::China, 0)
            .unwrap_err(),
        OAuthError::InvalidInput
    );
    for (access, refresh, cookie) in [
        ("", "refresh", None),
        ("access", "", None),
        ("bad\raccess", "refresh", None),
        ("access", "refresh", Some("\r\nprivate")),
        ("access", "refresh", Some(" ")),
    ] {
        assert_eq!(
            RefreshGrant::new(access, refresh, cookie).unwrap_err(),
            OAuthError::InvalidInput
        );
    }
    assert!(RefreshGrant::new(&"a".repeat(16_385), "refresh", None).is_err());
    assert!(RefreshGrant::new("access", &"r".repeat(16_385), None).is_err());
    assert!(RefreshGrant::new("access", "refresh", Some(&"c".repeat(16_385))).is_err());
}

#[test]
fn accio_capability_debug_redacts_client_state_code_verifier_and_tokens() {
    let auth = auth(LoginSite::China);
    let callback = callback(&auth);
    let shown = format!("{auth:?}");
    assert!(shown.contains("redacted"));
    assert!(!shown.contains("synthetic"));
    let grant = auth.finish(&callback).unwrap();
    let shown = format!("{grant:?}");
    assert!(!shown.contains(&grant.code));
    assert!(!shown.contains(&grant.verifier));
    assert!(
        !format!("{:?}", OAuthClient::new("private-client").unwrap()).contains("private-client")
    );
    let refresh =
        RefreshGrant::new("private-access", "private-refresh", Some("private-cookie")).unwrap();
    assert!(!format!("{refresh:?}").contains("private"));
}
