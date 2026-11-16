use super::{Authorization, OAuthError, RESOURCE};
use std::collections::HashMap;

const REDIRECT: &str = "http://127.0.0.1:34567/callback";

fn params(auth: &Authorization) -> HashMap<String, String> {
    url::Url::parse(&auth.authorize_url())
        .unwrap()
        .query_pairs()
        .into_owned()
        .collect()
}

pub(super) fn callback(auth: &Authorization) -> String {
    let mut url = url::Url::parse(auth.redirect_uri()).unwrap();
    url.query_pairs_mut().extend_pairs([
        ("state", params(auth)["state"].as_str()),
        ("code", "synthetic-code"),
    ]);
    url.into()
}

#[test]
fn luma_oauth_authorization_binds_resource_pkce_and_explicit_client() {
    let auth = Authorization::new("operator-client", REDIRECT).unwrap();
    let url = url::Url::parse(&auth.authorize_url()).unwrap();
    assert_eq!(
        url.origin().ascii_serialization(),
        "https://agents.lumalabs.ai"
    );
    assert_eq!(url.path(), "/mcp/oauth/authorize");
    let query = params(&auth);
    assert_eq!(query["client_id"], "operator-client");
    assert_eq!(query["redirect_uri"], REDIRECT);
    assert_eq!(query["scope"], "openid");
    assert_eq!(query["resource"], RESOURCE);
    assert_eq!(query["response_type"], "code");
    assert_eq!(query["code_challenge_method"], "S256");
    assert_eq!(query["state"].len(), 43);
    assert_eq!(query["code_challenge"].len(), 43);
    assert!(!query.contains_key("referrer"));
    assert!(auth.remaining().as_secs() <= 1200);
    let next = Authorization::new("operator-client", REDIRECT).unwrap();
    assert_ne!(params(&next)["state"], query["state"]);
    assert_ne!(params(&next)["code_challenge"], query["code_challenge"]);
    assert!(!format!("{auth:?}").contains(&query["state"]));
}

#[test]
fn luma_oauth_rejects_noncanonical_redirects_and_invalid_clients() {
    for redirect in [
        "http://localhost:34567/callback",
        "http://127.0.0.1:0/callback",
        "http://127.0.0.1/callback",
        "https://127.0.0.1:34567/callback",
        "http://127.0.0.1:34567/other",
        "http://user@127.0.0.1:34567/callback",
        "http://127.0.0.1:34567/callback?x=1",
        "http://127.0.0.1:34567/callback#fragment",
        "http://example.com:34567/callback",
    ] {
        assert!(matches!(
            Authorization::new("client", redirect),
            Err(OAuthError::InvalidInput)
        ));
    }
    for client in ["", "with space", "with\nnewline"] {
        assert!(matches!(
            Authorization::new(client, REDIRECT),
            Err(OAuthError::InvalidInput)
        ));
    }
}

#[test]
fn luma_oauth_callback_is_exact_and_wrong_state_does_not_consume_attempt() {
    let auth = Authorization::new("client", REDIRECT).unwrap();
    let valid = callback(&auth);
    for invalid in [
        valid.replace("34567", "34568"),
        format!("{valid}&state=duplicate"),
        format!("{valid}&code=duplicate"),
        format!("{valid}&error=access_denied"),
        format!("{valid}&iss=https%3A%2F%2Fauth.lumalabs.ai"),
        format!("{valid}#fragment"),
    ] {
        assert!(auth.validate_callback(&invalid).is_err());
    }
    assert_eq!(
        auth.validate_callback(&format!("{REDIRECT}?state=wrong&code=code")),
        Err(OAuthError::StateMismatch)
    );
    assert!(auth.validate_callback(&valid).is_ok());
    let grant = auth.into_code_grant(&valid).unwrap();
    assert!(!format!("{grant:?}").contains("synthetic-code"));
}

#[test]
fn luma_oauth_denials_do_not_echo_provider_text() {
    let auth = Authorization::new("client", REDIRECT).unwrap();
    let state = &params(&auth)["state"];
    assert_eq!(
        auth.validate_callback(&format!("{REDIRECT}?state={state}&error=access_denied")),
        Err(OAuthError::AccessDenied)
    );
    let error = auth
        .validate_callback(&format!("{REDIRECT}?state={state}&error=private-secret"))
        .unwrap_err();
    assert_eq!(error, OAuthError::Rejected);
    assert!(!error.to_string().contains("private-secret"));
}

#[tokio::test(start_paused = true)]
async fn luma_oauth_expired_callback_cannot_create_grant() {
    let auth = Authorization::new("client", REDIRECT).unwrap();
    let url = callback(&auth);
    tokio::time::advance(std::time::Duration::from_secs(1201)).await;
    assert_eq!(auth.validate_callback(&url), Err(OAuthError::Expired));
}
