//! Authentication rejection, audience isolation and RFC handshake admission, without credentials.
use super::{test_support as wire, *};
use serde_json::json;
#[test]
fn grok_web_boundary_cookie_identity_and_nonce_are_checked() {
    for token in ["", "a;b=c", "a b", "a\r\nX:leak"] {
        let mut payload = wire::payload();
        payload.api_key = token.into();
        assert!(session::headers(&payload).is_err());
    }
    for cookie in [
        "sso=another",
        "sso=synthetic-exact; sso=synthetic-exact",
        "sso=synthetic-exact; bad",
    ] {
        let mut payload = wire::payload();
        payload.headers.insert("Cookie".into(), cookie.into());
        assert!(session::headers(&payload).is_err());
    }
    for status in ["blocked", "unauthenticated"] {
        assert!(session::user_id(
            json!({"status":status,"session":{"userId":"00000000-0000-4000-8000-000000000001"}})
                .to_string()
                .as_bytes()
        )
        .is_err());
    }
    assert!(session::user_id(br#"{"email":"same@example.invalid"}"#).is_err());
    let headers = session::headers(&wire::payload()).unwrap();
    assert!(headers["cookie"].is_sensitive());
    assert!(!format!("{headers:?}").contains("synthetic-exact"));
    let key = "dGhlIHNhbXBsZSBub25jZQ==";
    let mut headers = rquest::header::HeaderMap::new();
    headers.insert("connection", "keep-alive, Upgrade".parse().unwrap());
    headers.insert("upgrade", "websocket".parse().unwrap());
    headers.insert(
        "sec-websocket-accept",
        "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=".parse().unwrap(),
    );
    assert!(dial::validate_handshake(&headers, key).is_ok());
    assert!(dial::validate_handshake(&headers, "foreign").is_err());
    headers.insert(
        "sec-websocket-extensions",
        "permessage-deflate".parse().unwrap(),
    );
    assert!(dial::validate_handshake(&headers, key).is_err());
}
#[tokio::test]
async fn grok_web_boundary_auth_rate_redirect_rejections_never_replay_or_follow() {
    for (identity_status, handshake_status, expected, calls) in [
        (401, 101, "grok_web_authentication", 1),
        (403, 101, "grok_web_authentication", 1),
        (429, 101, "grok_web_rate_limit", 1),
        (302, 101, "grok_web_redirect", 1),
        (200, 401, "grok_web_authentication", 2),
        (200, 429, "grok_web_rate_limit", 2),
        (200, 302, "grok_web_redirect", 2),
    ] {
        let server = wire::Server::start(wire::Reply {
            identity_status,
            handshake_status,
            ..Default::default()
        })
        .await;
        let error = wire::within(
            &server.origin,
            execute(
                &server.client(),
                std::time::Duration::from_secs(2),
                &wire::payload(),
                &wire::request(),
                "fast",
            ),
        )
        .await
        .unwrap_err();
        assert_eq!(error.code.as_deref(), Some(expected));
        assert_eq!(server.calls().len(), calls);
        assert!(server.events().is_empty());
        assert!(!error.message.contains("synthetic"));
        server.finish().await;
    }
}
