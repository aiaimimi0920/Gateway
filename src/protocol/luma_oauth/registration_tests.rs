use super::*;
use crate::protocol::luma_oauth::transport_fixture::{http, Mock, Reply as HttpReply};

const REDIRECT: &str = "http://127.0.0.1:54421/callback";

#[tokio::test]
async fn luma_registration_offline_requires_explicit_exact_grants() {
    let mut response = response();
    assert!(parse_with_offline(&serde_json::to_vec(&response).unwrap(), REDIRECT, true).is_err());
    response["grant_types"] = json!(["refresh_token", "authorization_code"]);
    let peer = Mock::new(HttpReply {
        status: 201,
        body: response.to_string(),
        ..Default::default()
    })
    .await;
    RegistrationRequest::new(REDIRECT)
        .unwrap()
        .with_offline_access()
        .register_at(&http(), &peer.endpoint, Duration::from_secs(2))
        .await
        .unwrap();
    assert_eq!(peer.count(), 1);
    {
        let calls = peer.calls.lock().unwrap();
        let body: serde_json::Value = serde_json::from_str(&calls[0].1).unwrap();
        assert_eq!(
            body["grant_types"],
            json!(["authorization_code", "refresh_token"])
        );
    }
    for grants in [
        json!(["authorization_code"]),
        json!(["refresh_token", "refresh_token"]),
        json!(["authorization_code", "refresh_token", "client_credentials"]),
    ] {
        response["grant_types"] = grants;
        assert!(
            parse_with_offline(&serde_json::to_vec(&response).unwrap(), REDIRECT, true).is_err()
        );
    }
    peer.stop().await;
}
fn response() -> serde_json::Value {
    json!({"client_id":"synthetic-client","redirect_uris":[REDIRECT],
        "grant_types":["authorization_code"],"response_types":["code"],
        "token_endpoint_auth_method":"none","registration_access_token":"must-not-retain"})
}
fn reply() -> HttpReply {
    HttpReply {
        status: 201,
        body: response().to_string(),
        ..Default::default()
    }
}

#[test]
fn luma_registration_rejects_unbound_redirects() {
    for redirect in [
        "https://evil.invalid/callback",
        "http://localhost:54421/callback",
        "http://127.0.0.1:0/callback",
        "http://127.0.0.1:54421/other",
        "http://127.0.0.1:54421/callback?x=1",
    ] {
        assert!(matches!(
            RegistrationRequest::new(redirect),
            Err(OAuthError::InvalidInput)
        ));
    }
}

#[test]
fn luma_registration_validates_identity_and_exact_public_contract() {
    let parsed = parse(&serde_json::to_vec(&response()).unwrap(), REDIRECT).unwrap();
    assert_eq!(parsed.client_id, "synthetic-client");
    assert_eq!(parsed.redirect_uri, REDIRECT);
    assert!(!format!("{parsed:?}").contains("must-not-retain"));
    for (key, value) in [
        ("client_id", json!("")),
        ("client_id", json!("x\ny")),
        ("client_id", json!("a".repeat(513))),
        ("redirect_uris", json!(["http://127.0.0.1:54422/callback"])),
        ("redirect_uris", json!([REDIRECT, "https://evil.invalid"])),
        (
            "grant_types",
            json!(["authorization_code", "refresh_token"]),
        ),
        ("response_types", json!(["token"])),
        ("token_endpoint_auth_method", json!("client_secret_post")),
        ("client_secret", json!("private-secret")),
        ("client_secret", json!("")),
    ] {
        let mut bad = response();
        bad[key] = value;
        assert_eq!(
            parse(&serde_json::to_vec(&bad).unwrap(), REDIRECT),
            Err(OAuthError::InvalidResponse)
        );
    }
    let duplicate = response()
        .to_string()
        .replacen('{', "{\"client_id\":\"second\",", 1);
    assert_eq!(
        parse(duplicate.as_bytes(), REDIRECT),
        Err(OAuthError::InvalidResponse)
    );
    let mut absent = response();
    absent
        .as_object_mut()
        .unwrap()
        .remove("token_endpoint_auth_method");
    assert_eq!(
        parse(&serde_json::to_vec(&absent).unwrap(), REDIRECT),
        Err(OAuthError::InvalidResponse)
    );
}

#[tokio::test]
async fn luma_registration_sends_one_public_request_without_inherited_credentials() {
    let peer = Mock::new(reply()).await;
    let result = RegistrationRequest::new(REDIRECT)
        .unwrap()
        .register_at(&http(), &peer.endpoint, Duration::from_secs(2))
        .await
        .unwrap();
    assert_eq!(result.client_id, "synthetic-client");
    {
        let calls = peer.calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        for name in ["authorization", "cookie", "x-api-key"] {
            assert!(!calls[0].0.contains_key(name));
        }
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&calls[0].1).unwrap(),
            json!({
            "client_name":"Neuro Gateway", "redirect_uris":[REDIRECT], "grant_types":["authorization_code"],
            "response_types":["code"], "token_endpoint_auth_method":"none" })
        );
    }
    peer.wait_body_dropped().await;
    peer.stop().await;
}

#[tokio::test]
async fn luma_registration_does_not_follow_redirects_or_retry_rejection() {
    let target = Mock::new(reply()).await;
    for status in [200, 302, 307, 400, 401, 429, 503] {
        let peer = Mock::new(HttpReply {
            status,
            location: Some(target.endpoint.clone()),
            body: "private-server-error".into(),
            ..reply()
        })
        .await;
        let error = RegistrationRequest::new(REDIRECT)
            .unwrap()
            .register_at(&http(), &peer.endpoint, Duration::from_secs(2))
            .await
            .unwrap_err();
        assert_eq!(
            error,
            if (300..400).contains(&status) {
                OAuthError::RedirectRejected
            } else {
                OAuthError::Rejected
            }
        );
        assert!(!error.to_string().contains("private-server-error"));
        assert_eq!(peer.count(), 1);
        assert_eq!(target.count(), 0);
        peer.stop().await;
    }
    target.stop().await;
}

#[tokio::test]
async fn luma_registration_bounds_decoded_body_and_rejects_malformed_reply() {
    for (body, expected) in [
        ("x".repeat(LIMIT + 1), OAuthError::ResponseTooLarge),
        ("<html>private</html>".into(), OAuthError::InvalidResponse),
    ] {
        let peer = Mock::new(HttpReply { body, ..reply() }).await;
        assert_eq!(
            RegistrationRequest::new(REDIRECT)
                .unwrap()
                .register_at(&http(), &peer.endpoint, Duration::from_secs(2))
                .await
                .unwrap_err(),
            expected
        );
        assert_eq!(peer.count(), 1);
        peer.stop().await;
    }
}

#[tokio::test]
async fn luma_registration_deadline_covers_headers_and_body() {
    for body in [false, true] {
        let peer = Mock::new(HttpReply {
            header_delay: if body {
                Duration::ZERO
            } else {
                Duration::from_millis(200)
            },
            body_delay: if body {
                Duration::from_millis(200)
            } else {
                Duration::ZERO
            },
            ..reply()
        })
        .await;
        let error = RegistrationRequest::new(REDIRECT)
            .unwrap()
            .register_at(&http(), &peer.endpoint, Duration::from_millis(50))
            .await
            .unwrap_err();
        assert!(matches!(error, OAuthError::Timeout | OAuthError::Transport));
        assert_eq!(peer.count(), 1);
        peer.stop().await;
    }
}

#[tokio::test]
async fn luma_registration_cancellation_releases_body_without_retry() {
    let peer = Mock::new(HttpReply {
        body_delay: Duration::from_secs(300),
        ..reply()
    })
    .await;
    let endpoint = peer.endpoint.clone();
    let task = tokio::spawn(async move {
        RegistrationRequest::new(REDIRECT)
            .unwrap()
            .register_at(&http(), &endpoint, BUDGET)
            .await
    });
    peer.wait_called().await;
    tokio::time::sleep(Duration::from_millis(50)).await;
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    peer.wait_body_dropped().await;
    assert_eq!(peer.count(), 1);
    peer.stop().await;
}
