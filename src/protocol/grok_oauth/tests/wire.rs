use super::*;
use axum::{extract::State, http::StatusCode, response::Response, routing::post, Router};
use std::sync::{Arc, Mutex};

type Calls = Arc<Mutex<Vec<(String, axum::http::HeaderMap)>>>;
pub(super) struct Fixture {
    pub(super) client: OAuthClient,
    pub(super) calls: Calls,
    server: tokio::task::JoinHandle<()>,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.server.abort();
    }
}
pub(super) async fn spawn_fixture(status: StatusCode, body: Value) -> Fixture {
    let calls: Calls = Arc::default();
    let data = (calls.clone(), status, body);
    let app = Router::new()
        .route("/oauth2/token", post(reply))
        .route("/oauth2/device/code", post(reply))
        .with_state(data);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let issuer = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    Fixture {
        client: client().test_issuer(&issuer),
        calls,
        server,
    }
}
async fn reply(
    State((calls, status, body)): State<(Calls, StatusCode, Value)>,
    headers: axum::http::HeaderMap,
    body_text: String,
) -> Response {
    calls.lock().unwrap().push((body_text, headers));
    Response::builder()
        .status(status)
        .header("content-type", "application/json")
        .body(axum::body::Body::from(body.to_string()))
        .unwrap()
}
pub(super) fn http() -> rquest::Client {
    rquest::Client::builder()
        .no_proxy()
        .default_headers({
            let mut h = rquest::header::HeaderMap::new();
            h.insert("authorization", "Bearer must-not-leak".parse().unwrap());
            h.insert("cookie", "must-not-leak".parse().unwrap());
            h
        })
        .build()
        .unwrap()
}
fn form(body: &str) -> std::collections::HashMap<String, String> {
    url::form_urlencoded::parse(body.as_bytes())
        .into_owned()
        .collect()
}

#[tokio::test]
async fn code_and_refresh_send_exact_grants_without_inherited_credentials() {
    let fixture = spawn_fixture(
        StatusCode::OK,
        json!({"access_token":"new-access","refresh_token":"rotated-refresh","expires_in":90}),
    )
    .await;
    let client = fixture
        .client
        .clone()
        .with_principal(Principal::new("Team", "team-123").unwrap());
    let auth = Authorization::new(&client, "http://127.0.0.1:23456/callback").unwrap();
    let result = client
        .exchange_code(&http(), &auth, "private-code&x=y")
        .await
        .unwrap();
    assert_eq!(result.refresh_token.as_deref(), Some("rotated-refresh"));
    client
        .refresh(&http(), "private-refresh&x=y")
        .await
        .unwrap();
    let calls = fixture.calls.lock().unwrap();
    assert_eq!(calls.len(), 2);
    let first = form(&calls[0].0);
    assert_eq!(first.len(), 5);
    assert_eq!(first["grant_type"], "authorization_code");
    assert_eq!(first["code"], "private-code&x=y");
    assert_eq!(first["redirect_uri"], auth.redirect_uri);
    assert_eq!(first["code_verifier"], auth.verifier);
    let second = form(&calls[1].0);
    assert_eq!(second.len(), 5);
    assert_eq!(second["grant_type"], "refresh_token");
    assert_eq!(second["refresh_token"], "private-refresh&x=y");
    assert_eq!(second["principal_type"], "Team");
    assert_eq!(second["principal_id"], "team-123");
    for (_, headers) in calls.iter() {
        assert!(!headers.contains_key("authorization"));
        assert!(!headers.contains_key("cookie"));
        assert!(!headers.contains_key("x-xai-token-auth"));
    }
}

#[tokio::test]
async fn errors_are_typed_fixed_and_never_retry_the_grant() {
    for (kind, expected) in [
        ("invalid_grant", OAuthError::ReauthorizationRequired),
        ("invalid_client", OAuthError::ReauthorizationRequired),
        ("access_denied", OAuthError::AccessDenied),
        ("expired_token", OAuthError::Expired),
        ("unknown-private-code", OAuthError::Rejected),
    ] {
        let fixture = spawn_fixture(
            StatusCode::BAD_REQUEST,
            json!({"error":kind,"error_description":"private-upstream-prose"}),
        )
        .await;
        let error = fixture
            .client
            .refresh(&http(), "private-refresh")
            .await
            .unwrap_err();
        assert_eq!(error, expected);
        assert!(!format!("{error:?} {error}").contains("private-"));
        assert_eq!(fixture.calls.lock().unwrap().len(), 1);
    }
}

#[tokio::test]
async fn malformed_success_and_oversize_body_do_not_become_tokens() {
    for body in [
        json!({}),
        json!({"access_token":"access","error":"invalid_grant"}),
        json!({"access_token":"x","padding":"p".repeat(65536)}),
    ] {
        let fixture = spawn_fixture(StatusCode::OK, body).await;
        assert!(fixture
            .client
            .refresh(&http(), "private-refresh")
            .await
            .is_err());
        assert_eq!(fixture.calls.lock().unwrap().len(), 1);
    }
    let fixture = spawn_fixture(StatusCode::OK, json!({"access_token":"ok"})).await;
    assert_eq!(
        fixture
            .client
            .refresh(&http(), "bad\nrefresh")
            .await
            .unwrap_err(),
        OAuthError::InvalidInput
    );
    assert!(fixture.calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn device_issue_and_poll_preserve_pending_slowdown_and_terminal_errors() {
    let fixture=spawn_fixture(StatusCode::OK,json!({"device_code":"private-device","user_code":"ABCD","verification_uri":"https://accounts.x.ai/device","expires_in":60,"interval":1})).await;
    let _device = fixture.client.begin_device(&http()).await.unwrap();
    let issue = form(&fixture.calls.lock().unwrap()[0].0);
    assert_eq!(issue.len(), 3);
    assert_eq!(issue["referrer"], "neuro-gateway");
    for (kind, expected) in [
        ("authorization_pending", DevicePoll::Pending),
        ("slow_down", DevicePoll::SlowDown),
    ] {
        let fixture = spawn_fixture(StatusCode::BAD_REQUEST, json!({"error":kind})).await;
        let mut device=DeviceChallenge::parse(json!({"device_code":"private-device","user_code":"ABCD","verification_uri":"https://accounts.x.ai/device","expires_in":60,"interval":1}), &fixture.client).unwrap();
        assert_eq!(
            fixture
                .client
                .poll_device(&http(), &mut device)
                .await
                .unwrap(),
            expected
        );
        assert_eq!(
            device.interval.as_secs(),
            if kind == "slow_down" { 6 } else { 1 }
        );
        let poll = form(&fixture.calls.lock().unwrap()[0].0);
        assert_eq!(poll.len(), 3);
        assert_eq!(
            poll["grant_type"],
            "urn:ietf:params:oauth:grant-type:device_code"
        );
        assert_eq!(poll["device_code"], "private-device");
    }
}
