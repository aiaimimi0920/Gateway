//! Reuse the existing owned HTTP fixture, explicitly replacing its product-specific default reply.
use super::transport_test_support::{test_body_started, test_budget, test_endpoint};
use super::{
    authorization_tests::callback,
    material_tests::tokens,
    transport_fixture::{http, Mock, Reply},
    *,
};
use std::time::Duration;
use time::OffsetDateTime;

fn reply() -> Reply {
    Reply {
        body: tokens().to_string(),
        ..Reply::default()
    }
}
fn grant() -> RefreshGrant {
    RefreshGrant::new(
        OAuthClient::new("explicit-client", Some("client-private")).unwrap(),
        "refresh-private",
        None,
    )
    .unwrap()
}
async fn refresh(mock: &Mock, grant: RefreshGrant) -> Result<TokenMaterial, OAuthError> {
    test_endpoint(mock.endpoint.clone(), grant.execute(&http())).await
}

#[tokio::test]
async fn vertex_oauth_transport_code_and_refresh_use_owned_form_and_strip_default_secrets() {
    for (code, with_secret) in [(true, true), (true, false), (false, true), (false, false)] {
        let mock = Mock::new(reply()).await;
        let client =
            OAuthClient::new("explicit-client", with_secret.then_some("client-private")).unwrap();
        let token = if code {
            let auth = AuthorizationAttempt::new(client, 43991).unwrap();
            let verifier = auth.verifier.clone();
            let callback = callback(&auth);
            let code = auth.finish(&callback).unwrap();
            let token = test_endpoint(mock.endpoint.clone(), code.exchange(&http()))
                .await
                .unwrap();
            let calls = mock.calls.lock().unwrap();
            let form: std::collections::HashMap<_, _> =
                url::form_urlencoded::parse(calls[0].1.as_bytes()).collect();
            assert_eq!(form["grant_type"], "authorization_code");
            assert_eq!(form["code_verifier"], verifier);
            assert_eq!(form["code"], "synthetic-code");
            assert_eq!(form["redirect_uri"], "http://127.0.0.1:43991/");
            assert!(!form.contains_key("refresh_token"));
            token
        } else {
            refresh(
                &mock,
                RefreshGrant::new(client, "refresh-private", None).unwrap(),
            )
            .await
            .unwrap()
        };
        assert_eq!(token.access_token(), "access-private");
        assert_eq!(mock.count(), 1);
        {
            let calls = mock.calls.lock().unwrap();
            let (headers, body) = &calls[0];
            assert!(headers["content-type"]
                .to_str()
                .unwrap()
                .starts_with("application/x-www-form-urlencoded"));
            for name in ["authorization", "cookie", "x-api-key"] {
                assert!(!headers.contains_key(name));
            }
            let form: std::collections::HashMap<_, _> =
                url::form_urlencoded::parse(body.as_bytes()).collect();
            assert_eq!(
                form.get("client_secret").map(|value| value.as_ref()),
                with_secret.then_some("client-private")
            );
            if !code {
                assert_eq!(form["grant_type"], "refresh_token");
                assert_eq!(form["refresh_token"], "refresh-private");
                assert!(!form.contains_key("code_verifier"));
            }
        }
        mock.stop().await;
    }
}

#[tokio::test]
async fn vertex_oauth_transport_missing_rotated_refresh_preserves_original_absolute_deadline() {
    let mut value = tokens();
    value.as_object_mut().unwrap().remove("refresh_token");
    value["refresh_token_expires_in"] = serde_json::json!(9000);
    let mock = Mock::new(Reply {
        body: value.to_string(),
        ..reply()
    })
    .await;
    let original = OffsetDateTime::now_utc() + time::Duration::seconds(700);
    let grant = RefreshGrant::new(
        OAuthClient::new("explicit", None).unwrap(),
        "refresh-private",
        Some(original),
    )
    .unwrap();
    let token = refresh(&mock, grant).await.unwrap();
    assert_eq!(token.refresh_token(), Some("refresh-private"));
    assert_eq!(token.refresh_expires_at(), Some(original));
    assert!(!mock.calls.lock().unwrap()[0].1.contains("client_secret"));
    mock.stop().await;
}

#[tokio::test]
async fn vertex_oauth_transport_redirect_never_replays_form_or_leaks_credentials() {
    let destination = Mock::new(reply()).await;
    for status in [301, 302, 303, 307, 308] {
        let source = Mock::new(Reply {
            status,
            location: Some(destination.endpoint.clone()),
            ..reply()
        })
        .await;
        assert_eq!(
            refresh(&source, grant()).await.unwrap_err(),
            OAuthError::RedirectRejected
        );
        assert_eq!(source.count(), 1);
        assert_eq!(destination.count(), 0);
        source.stop().await;
    }
    destination.stop().await;
}

#[tokio::test]
async fn vertex_oauth_transport_error_classification_is_fixed_and_never_uses_prose() {
    for (status, error, expected) in [
        (400, "invalid_grant", OAuthError::ReauthorizationRequired),
        (401, "invalid_client", OAuthError::ReauthorizationRequired),
        (400, "access_denied", OAuthError::AccessDenied),
        (429, "private-prose", OAuthError::Rejected),
        (500, "invalid_grant", OAuthError::Rejected),
    ] {
        let mock = Mock::new(Reply {
            status,
            body: serde_json::json!({"error":error,"error_description":"private-diagnostic"})
                .to_string(),
            ..reply()
        })
        .await;
        assert_eq!(refresh(&mock, grant()).await.unwrap_err(), expected);
        assert_eq!(mock.count(), 1);
        assert!(!format!("{expected:?}").contains("private"));
        mock.stop().await;
    }
}

#[tokio::test]
async fn vertex_oauth_transport_bounds_decoded_reply_and_has_one_send_read_deadline() {
    let mock = Mock::new(Reply {
        body: "x".repeat(65_537),
        ..reply()
    })
    .await;
    assert_eq!(
        refresh(&mock, grant()).await.unwrap_err(),
        OAuthError::ResponseTooLarge
    );
    mock.stop().await;
    let mock = Mock::new(Reply {
        body_delay: Duration::from_secs(10),
        ..reply()
    })
    .await;
    let result = test_budget(Duration::from_millis(100), refresh(&mock, grant())).await;
    assert_eq!(result.unwrap_err(), OAuthError::Timeout);
    mock.wait_body_dropped().await;
    assert_eq!(mock.count(), 1);
    mock.stop().await;
}

#[tokio::test]
async fn vertex_oauth_transport_cancellation_drops_response_without_retry_or_late_result() {
    let mock = Mock::new(Reply {
        body_delay: Duration::from_secs(10),
        ..reply()
    })
    .await;
    let started = std::sync::Arc::new(tokio::sync::Notify::new());
    {
        let future = test_body_started(started.clone(), refresh(&mock, grant()));
        tokio::pin!(future);
        tokio::select! {
            value=&mut future=>panic!("unexpected early result: {value:?}"),
            _=started.notified()=>{},
            _=tokio::time::sleep(Duration::from_secs(2))=>panic!("body reader never started"),
        }
    }
    tokio::time::timeout(Duration::from_secs(2), mock.wait_body_dropped())
        .await
        .unwrap();
    assert_eq!(mock.count(), 1);
    mock.stop().await;
}
