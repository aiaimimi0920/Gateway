use super::super::transport::exchange;
use super::*;

#[tokio::test]
async fn antigravity_rejects_custom_endpoints_without_sending_tokens_or_invalidating_account() {
    let mock = MockOAuth::new(200, ROTATED, 0).await;
    let Err(error) = exchange(&client(), &mock.url, "secret-refresh", "client", true).await else {
        panic!("must reject")
    };
    assert_eq!(error.code, "antigravity_oauth_client_mismatch");
    assert!(!error.terminal);
    assert_eq!(mock.count(), 0);
}

#[tokio::test]
async fn malformed_and_oversized_responses_preserve_secret_safe_errors() {
    for body in [
        "not-json".to_string(),
        "x".repeat(65_537),
        r#"{"access_token":"","expires_in":3600}"#.to_string(),
        r#"{"access_token":"fixture","expires_in":0}"#.to_string(),
    ] {
        let mock = MockOAuth::new(200, &body, 0).await;
        let result = exchange(&client(), &mock.url, "secret-refresh", "client", false).await;
        let Err(error) = result else {
            panic!("expected invalid OAuth response");
        };
        assert!(!error.code.contains("secret-refresh"));
        assert!(!error.terminal);
    }
}

#[tokio::test]
async fn invalid_grant_requires_authorization_but_rate_limit_retries() {
    for (status, body, terminal) in [
        (
            400,
            r#"{"error":"invalid_grant","description":"secret-refresh"}"#,
            true,
        ),
        (429, r#"{"error":"rate_limit"}"#, false),
        (503, "temporary failure", false),
    ] {
        let mock = MockOAuth::new(status, body, 0).await;
        let Err(error) = exchange(&client(), &mock.url, "secret-refresh", "client", false).await
        else {
            panic!("expected rejection");
        };
        assert_eq!(error.terminal, terminal);
        assert!(!error.code.contains("secret-refresh"));
    }
}

#[tokio::test]
async fn unsafe_endpoints_are_rejected_before_network() {
    for url in [
        "http://example.com/token",
        "https://user:secret@example.com/token",
        "not a URL",
    ] {
        assert!(exchange(&client(), url, "secret-refresh", "client", false)
            .await
            .is_err());
    }
}

#[tokio::test]
async fn oauth_redirect_never_replays_form_to_another_endpoint() {
    let target = MockOAuth::new(200, ROTATED, 0).await;
    let url = target.url.clone();
    let app = axum::Router::new().route(
        "/token",
        axum::routing::post(move || async move {
            (
                axum::http::StatusCode::TEMPORARY_REDIRECT,
                [("location", url)],
            )
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/token", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    assert!(exchange(&client(), &url, "secret-refresh", "client", false)
        .await
        .is_err());
    task.abort();
    assert_eq!(target.count(), 0);
}
