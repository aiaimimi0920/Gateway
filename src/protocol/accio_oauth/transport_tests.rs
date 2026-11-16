//! The real HTTP owner must send the exact JSON wire, not OAuth form or default credentials.
use super::{authorization_tests::callback, transport, transport_fixture::*, *};
use serde_json::{json, Value};

#[tokio::test]
async fn accio_code_exchange_sends_bound_pkce_client_redirect_as_json_once() {
    let mock = Mock::new(Reply::default()).await;
    let client = OAuthClient::new("explicit-synthetic-client").unwrap();
    let auth = AuthorizationAttempt::new(client, LoginSite::Global, 34567).unwrap();
    let callback = callback(&auth);
    let grant = auth.finish(&callback).unwrap();
    let verifier = grant.verifier.clone();
    let tokens = transport::test_exchange(grant, &http(), &mock.endpoint)
        .await
        .unwrap();
    assert_eq!(tokens.access_token(), "rotated-synthetic-access");
    assert_eq!(tokens.refresh_token(), "rotated-synthetic-refresh");
    assert_eq!(tokens.sgcookie(), Some("rotated-cookie"));
    assert_eq!(tokens.expires_at().unix_timestamp(), 1790000000);
    let calls = mock.calls.lock().unwrap().clone();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].0["content-type"], "application/json");
    assert_eq!(calls[0].0["accept"], "application/json");
    for name in ["authorization", "cookie", "x-api-key"] {
        assert!(!calls[0].0.contains_key(name));
    }
    assert_eq!(
        serde_json::from_str::<Value>(&calls[0].1).unwrap(),
        json!({"code":"synthetic-code",
        "codeVerifier":verifier,"clientId":"explicit-synthetic-client","redirectUri":"http://127.0.0.1:34567/auth/callback"})
    );
    mock.stop().await;
}

#[tokio::test]
async fn accio_refresh_sends_only_pinned_current_access_and_refresh_not_cookie_or_client_form() {
    let mock = Mock::new(Reply::default()).await;
    let grant = RefreshGrant::new("current-access", "current-refresh", Some("old-cookie")).unwrap();
    let tokens = transport::test_refresh(grant, &http(), &mock.endpoint)
        .await
        .unwrap();
    assert_eq!(tokens.sgcookie(), Some("rotated-cookie"));
    let calls = mock.calls.lock().unwrap().clone();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].0["content-type"], "application/json");
    for name in ["authorization", "cookie", "x-api-key"] {
        assert!(!calls[0].0.contains_key(name));
    }
    assert_eq!(
        serde_json::from_str::<Value>(&calls[0].1).unwrap(),
        json!({"accessToken":"current-access","refreshToken":"current-refresh"})
    );
    mock.stop().await;
}

#[tokio::test]
async fn accio_refresh_preserves_old_cookie_when_upstream_omits_new_cookie() {
    let mut value = rotated();
    value["data"].as_object_mut().unwrap().remove("sgcookie");
    let mock = Mock::new(Reply {
        body: value.to_string(),
        ..Default::default()
    })
    .await;
    let grant =
        RefreshGrant::new("current-access", "current-refresh", Some("original-cookie")).unwrap();
    let tokens = transport::test_refresh(grant, &http(), &mock.endpoint)
        .await
        .unwrap();
    assert_eq!(tokens.sgcookie(), Some("original-cookie"));
    assert_eq!(mock.count(), 1);
    mock.stop().await;
}

#[tokio::test]
async fn accio_http_failures_and_false_or_incomplete_envelopes_never_retry_or_return_material() {
    for (status, body, expected) in [
        (
            200,
            json!({"success":false,"code":"401","message":"private-access"}).to_string(),
            OAuthError::Rejected,
        ),
        (401, rotated().to_string(), OAuthError::Rejected),
        (429, rotated().to_string(), OAuthError::Rejected),
        (500, "private-upstream-text".into(), OAuthError::Rejected),
        (
            200,
            "private-upstream-text".into(),
            OAuthError::InvalidResponse,
        ),
        (
            200,
            json!({"success":true,"data":{"accessToken":"private-access"}}).to_string(),
            OAuthError::InvalidResponse,
        ),
    ] {
        let mock = Mock::new(Reply {
            status,
            body,
            ..Default::default()
        })
        .await;
        let grant = RefreshGrant::new("current-access", "current-refresh", None).unwrap();
        let error = transport::test_refresh(grant, &http(), &mock.endpoint)
            .await
            .unwrap_err();
        assert_eq!(error, expected);
        assert!(!error.to_string().contains("private"));
        assert_eq!(
            mock.count(),
            1,
            "an ambiguous rotating grant must not retry"
        );
        mock.stop().await;
    }
}

#[tokio::test]
async fn accio_expired_code_capability_is_rejected_without_any_send() {
    let mock = Mock::new(Reply::default()).await;
    let auth = AuthorizationAttempt::new(
        OAuthClient::new("synthetic").unwrap(),
        LoginSite::China,
        34567,
    )
    .unwrap();
    let callback = callback(&auth);
    let mut grant = auth.finish(&callback).unwrap();
    grant.deadline = std::time::Instant::now();
    assert_eq!(
        transport::test_exchange(grant, &http(), &mock.endpoint)
            .await
            .unwrap_err(),
        OAuthError::Expired
    );
    assert_eq!(mock.count(), 0);
    mock.stop().await;
}
