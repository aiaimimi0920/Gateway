//! Reuse the existing owned loopback fixture, not a second HTTP testing framework.
use super::*;
use crate::protocol::luma_oauth::{tests::callback, transport_fixture::*, Authorization};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

fn auth() -> Authorization {
    Authorization::new("explicit-client", "http://127.0.0.1:34567/callback").unwrap()
}
fn grant() -> CodeGrant {
    let auth = auth();
    let url = callback(&auth);
    auth.into_code_grant(&url).unwrap()
}
fn reply(body: &str) -> Reply {
    Reply {
        body: body.into(),
        ..Default::default()
    }
}
const VALID: &str = r#"{"access_token":"synthetic-token","token_type":"Bearer","expires_in":3600}"#;

#[tokio::test]
async fn luma_oauth_exchange_sends_exact_form_without_default_credentials() {
    let mock = Mock::new(reply(VALID)).await;
    let auth = auth();
    let challenge = url::Url::parse(&auth.authorize_url())
        .unwrap()
        .query_pairs()
        .find(|(key, _)| key == "code_challenge")
        .unwrap()
        .1
        .into_owned();
    let callback = callback(&auth);
    let before = OffsetDateTime::now_utc();
    let material = auth
        .into_code_grant(&callback)
        .unwrap()
        .exchange_at(&http(), &mock.endpoint, DEADLINE)
        .await
        .unwrap();
    assert_eq!(material.access_token(), "synthetic-token");
    assert_eq!(material.client_id(), "explicit-client");
    assert_eq!(material.redirect_uri(), "http://127.0.0.1:34567/callback");
    assert!(material.expires_at().unwrap() >= before + time::Duration::seconds(3599));
    assert!(
        material.expires_at().unwrap() <= OffsetDateTime::now_utc() + time::Duration::seconds(3600)
    );
    assert!(!format!("{material:?}").contains("synthetic-token"));
    {
        let calls = mock.calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        for header in ["authorization", "cookie", "x-api-key"] {
            assert!(!calls[0].0.contains_key(header));
        }
        let form: HashMap<String, String> = url::form_urlencoded::parse(calls[0].1.as_bytes())
            .into_owned()
            .collect();
        assert_eq!(form.len(), 6);
        assert_eq!(form["grant_type"], "authorization_code");
        assert_eq!(form["code"], "synthetic-code");
        assert_eq!(form["client_id"], "explicit-client");
        assert_eq!(form["redirect_uri"], "http://127.0.0.1:34567/callback");
        assert_eq!(form["resource"], RESOURCE);
        assert_eq!(
            URL_SAFE_NO_PAD.encode(Sha256::digest(form["code_verifier"].as_bytes())),
            challenge
        );
    }
    mock.stop().await;
}

#[tokio::test]
async fn luma_oauth_unknown_expiry_stays_unknown_without_refresh_or_identity_claims() {
    let mock = Mock::new(reply(r#"{"access_token":"opaque-token","token_type":"bearer","id_token":"unverified","refresh_token":"unused"}"#)).await;
    let material = grant()
        .exchange_at(&http(), &mock.endpoint, DEADLINE)
        .await
        .unwrap();
    assert!(material.expires_at().is_none());
    assert!(!format!("{material:?}").contains("unused"));
    mock.stop().await;
}

#[tokio::test]
async fn luma_oauth_rejects_bad_and_duplicate_token_fields_without_echo_or_retry() {
    for body in [
        r#"{"access_token":"secret","token_type":"Basic"}"#,
        r#"{"access_token":"secret","token_type":"Bearer","expires_in":0}"#,
        r#"{"access_token":"secret","token_type":"Bearer","expires_in":-1}"#,
        r#"{"access_token":"secret","token_type":"Bearer","expires_in":"3600"}"#,
        r#"{"access_token":"secret","token_type":"Bearer","access_token":"other"}"#,
        r#"{"access_token":"secret","token_type":"Bearer","expires_in":1,"expires_in":3600}"#,
        r#"{"access_token":"secret injected","token_type":"Bearer"}"#,
        r#"{"access_token":"secret","token_type":"Bearer","error":"private-error"}"#,
        "not-json-private-error",
    ] {
        let mock = Mock::new(reply(body)).await;
        let error = grant()
            .exchange_at(&http(), &mock.endpoint, DEADLINE)
            .await
            .unwrap_err();
        assert_eq!(error, OAuthError::InvalidResponse);
        assert!(!error.to_string().contains("secret"));
        assert_eq!(mock.count(), 1);
        mock.stop().await;
    }
}

#[tokio::test]
async fn luma_oauth_redirects_and_http_errors_never_retry_or_leak_bodies() {
    let destination = Mock::new(reply(VALID)).await;
    for status in [301, 302, 303, 307, 308, 400, 401, 429, 503] {
        let mock = Mock::new(Reply {
            status,
            location: Some(destination.endpoint.clone()),
            ..reply("private-provider-error")
        })
        .await;
        let error = grant()
            .exchange_at(&http(), &mock.endpoint, DEADLINE)
            .await
            .unwrap_err();
        assert_eq!(
            error,
            if status < 400 {
                OAuthError::RedirectRejected
            } else {
                OAuthError::Rejected
            }
        );
        assert_eq!(mock.count(), 1);
        assert_eq!(destination.count(), 0);
        mock.stop().await;
    }
    destination.stop().await;
}

#[tokio::test]
async fn luma_oauth_decoded_body_budget_is_enforced() {
    let mock = Mock::new(reply(&"x".repeat(LIMIT + 1))).await;
    assert_eq!(
        grant()
            .exchange_at(&http(), &mock.endpoint, DEADLINE)
            .await
            .unwrap_err(),
        OAuthError::ResponseTooLarge
    );
    assert_eq!(mock.count(), 1);
    mock.stop().await;
}

#[tokio::test]
async fn luma_oauth_deadline_covers_headers_and_body() {
    for (header_delay, body_delay) in [
        (Duration::from_millis(400), Duration::ZERO),
        (Duration::ZERO, Duration::from_millis(400)),
    ] {
        let mock = Mock::new(Reply {
            header_delay,
            body_delay,
            ..reply(VALID)
        })
        .await;
        let start = std::time::Instant::now();
        let error = grant()
            .exchange_at(&http(), &mock.endpoint, Duration::from_millis(100))
            .await
            .unwrap_err();
        assert!(matches!(error, OAuthError::Timeout | OAuthError::Transport));
        assert!(start.elapsed() < Duration::from_secs(3));
        assert_eq!(mock.count(), 1);
        mock.stop().await;
    }
}

#[tokio::test]
async fn luma_oauth_cancellation_releases_body_without_background_retry() {
    let mock = Mock::new(Reply {
        body_delay: Duration::from_secs(300),
        ..reply(VALID)
    })
    .await;
    let endpoint = mock.endpoint.clone();
    let task = tokio::spawn(async move { grant().exchange_at(&http(), &endpoint, DEADLINE).await });
    mock.wait_called().await;
    tokio::time::sleep(Duration::from_millis(50)).await;
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    mock.wait_body_dropped().await;
    assert_eq!(mock.count(), 1);
    mock.stop().await;
}

#[tokio::test(start_paused = true)]
async fn luma_oauth_expired_grant_never_sends() {
    let grant = grant();
    tokio::time::advance(Duration::from_secs(1201)).await;
    assert_eq!(
        grant.exchange(&http()).await.unwrap_err(),
        OAuthError::Expired
    );
}
