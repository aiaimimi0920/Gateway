use super::*;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use sha2::{Digest, Sha256};
use std::{collections::HashMap, time::Instant};
use url::Url;

fn client() -> OAuthClient {
    OAuthClient::new(
        "explicit-test.apps.googleusercontent.com",
        Some("client-private"),
    )
    .unwrap()
}
pub(super) fn attempt() -> AuthorizationAttempt {
    AuthorizationAttempt::new(client(), 43991).unwrap()
}
pub(super) fn callback(auth: &AuthorizationAttempt) -> String {
    format!(
        "{}?state={}&code=synthetic-code",
        auth.redirect_uri(),
        auth.state
    )
}

#[test]
fn vertex_oauth_client_requires_explicit_bounded_material_without_borrowing_identity() {
    for id in ["", "a b", "private\nvalue"] {
        assert_eq!(
            OAuthClient::new(id, None).unwrap_err(),
            OAuthError::InvalidInput
        );
    }
    assert!(OAuthClient::new(&"a".repeat(1025), None).is_err());
    for secret in ["", "a b", "a\r\nb"] {
        assert!(OAuthClient::new("explicit", Some(secret)).is_err());
    }
    assert!(OAuthClient::new("explicit", None).is_ok());
    let debug = format!("{:?}", client());
    assert!(!debug.contains("client-private") && !debug.contains("explicit-test"));
}

#[test]
fn vertex_oauth_authorization_uses_fixed_google_pkce_and_loopback_not_other_products() {
    assert!(AuthorizationAttempt::new(client(), 0).is_err());
    let first = attempt();
    let url = Url::parse(&first.authorize_url()).unwrap();
    assert_eq!(url.origin().ascii_serialization(), ISSUER);
    assert_eq!(url.path(), "/o/oauth2/v2/auth");
    let query: HashMap<_, _> = url
        .query_pairs()
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();
    assert_eq!(query["client_id"], client().client_id());
    assert_eq!(query["redirect_uri"], "http://127.0.0.1:43991/");
    assert_eq!(query["scope"], CLOUD_PLATFORM_SCOPE);
    assert_eq!(query["response_type"], "code");
    assert_eq!(query["access_type"], "offline");
    assert_eq!(query["prompt"], "consent");
    assert_eq!(query["code_challenge_method"], "S256");
    assert_eq!(
        query["code_challenge"],
        URL_SAFE_NO_PAD.encode(Sha256::digest(first.verifier.as_bytes()))
    );
    assert!((43..=128).contains(&first.verifier.len()));
    assert!(first
        .verifier
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-'));
    assert_ne!(first.state, attempt().state);
    for hidden in [
        "client-private",
        &first.verifier,
        "cclog",
        "aicode",
        "oob",
        "client_secret",
    ] {
        assert!(!url.as_str().contains(hidden));
    }
}

#[test]
fn vertex_oauth_callback_is_bound_to_state_exact_listener_and_one_owned_code() {
    let auth = attempt();
    let callback = format!(
        "{}&scope=untrusted&authuser=0&iss={ISSUER}",
        callback(&auth)
    );
    let grant = auth.finish(&callback).unwrap();
    assert_eq!(grant.code, "synthetic-code");
    assert_eq!(grant.client.client_id(), client().client_id());
    assert!(grant.remaining().as_secs() <= 600);
    for debug in [format!("{grant:?}"), format!("{:?}", attempt())] {
        for hidden in ["synthetic-code", "client-private", &grant.verifier] {
            assert!(!debug.contains(hidden));
        }
    }
}

#[test]
fn vertex_oauth_callback_rejects_state_origin_duplicates_and_smuggled_error() {
    for case in 0..12 {
        let auth = attempt();
        let callback = callback(&auth);
        let bad = match case {
            0 => callback.replace(&auth.state, "wrong"),
            1 => callback.replace("127.0.0.1", "localhost"),
            2 => callback.replace("43991", "43992"),
            3 => callback.replace("http:", "https:"),
            4 => callback.replace("127.0.0.1", "user@127.0.0.1"),
            5 => format!("{callback}#fragment"),
            6 => format!("{callback}&code=second"),
            7 => format!("{callback}&state=second"),
            8 => format!("{callback}&error=access_denied"),
            9 => format!("{callback}&iss=https://other.invalid"),
            10 => format!("{callback}&error_description=private-prose"),
            _ => format!("{callback}\n"),
        };
        assert!(auth.finish(&bad).is_err(), "case {case}");
    }
    let auth = attempt();
    let callback = format!(
        "{}?state={}&error=access_denied&error_description=private",
        auth.redirect_uri(),
        auth.state
    );
    assert_eq!(
        auth.finish(&callback).unwrap_err(),
        OAuthError::AccessDenied
    );
    let auth = attempt();
    let callback = format!("{}?code=private", auth.redirect_uri());
    assert_eq!(
        auth.finish(&callback).unwrap_err(),
        OAuthError::StateMismatch
    );
}

#[test]
fn vertex_oauth_expired_capabilities_cannot_start_code_or_refresh_exchange() {
    let mut auth = attempt();
    let callback = callback(&auth);
    auth.deadline = Instant::now();
    assert_eq!(auth.finish(&callback).unwrap_err(), OAuthError::Expired);
    assert_eq!(
        RefreshGrant::new(client(), "refresh", Some(time::OffsetDateTime::now_utc())).unwrap_err(),
        OAuthError::Expired
    );
    assert!(RefreshGrant::new(client(), "", None).is_err());
}

#[test]
fn vertex_oauth_callback_limits_unknown_metadata_without_using_it_as_identity() {
    for port in [80, 43991] {
        let auth = AuthorizationAttempt::new(client(), port).unwrap();
        let callback = callback(&auth);
        assert!(auth.finish(&callback).is_ok());
    }
    for tail in [
        "&extra=x&extra=y".to_owned(),
        "&scope=x".repeat(17),
        "&unknown=".to_owned() + &"x".repeat(16_384),
    ] {
        let auth = attempt();
        let callback = format!("{}{tail}", callback(&auth));
        assert_eq!(
            auth.finish(&callback).unwrap_err(),
            OAuthError::InvalidCallback
        );
    }
    let auth = attempt();
    let callback = format!("{}&metadata=not-an-identity", callback(&auth));
    assert!(auth.finish(&callback).is_ok());
}

#[cfg(not(feature = "line-google-agent-platform-official"))]
#[tokio::test]
async fn vertex_oauth_feature_off_refuses_code_and_refresh_before_network() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}/grant", listener.local_addr().unwrap());
    let http = rquest::Client::builder().no_proxy().build().unwrap();
    let auth = attempt();
    let callback = callback(&auth);
    let code = auth.finish(&callback).unwrap();
    let refresh = RefreshGrant::new(client(), "synthetic-refresh", None).unwrap();
    assert_eq!(
        super::transport_test_support::test_endpoint(endpoint.clone(), code.exchange(&http))
            .await
            .unwrap_err(),
        OAuthError::FeatureDisabled
    );
    assert_eq!(
        super::transport_test_support::test_endpoint(endpoint, refresh.execute(&http))
            .await
            .unwrap_err(),
        OAuthError::FeatureDisabled
    );
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(100), listener.accept())
            .await
            .is_err()
    );
}
