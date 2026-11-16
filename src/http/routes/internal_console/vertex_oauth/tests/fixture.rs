//! All exchanges use a test-only loopback peer, including tasks spawned for the real callback.
pub(super) use super::transport_fixture::{Mock, Reply};
use super::*;
pub(super) use crate::protocol::vertex_oauth::{CLOUD_PLATFORM_SCOPE, TOKEN_ENDPOINT};
use crate::{
    console::vertex_oauth::Session, protocol::vertex_oauth::transport_test_support::test_endpoint,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub(super) fn tokens() -> Value {
    json!({"access_token":"synthetic-new-access","refresh_token":"synthetic-new-refresh",
        "token_type":"Bearer","expires_in":3600,"refresh_token_expires_in":7200,"scope":CLOUD_PLATFORM_SCOPE})
}
pub(super) async fn issuer() -> Mock {
    Mock::new(Reply {
        body: tokens().to_string(),
        ..Reply::default()
    })
    .await
}
pub(super) fn session(state: &Arc<AppState>, view: &Value) -> Arc<Session> {
    let (owner, _) = super::super::authorize(
        state,
        &axum::http::HeaderMap::new(),
        Some(support::MANAGEMENT_TOKEN),
        None,
        false,
    )
    .unwrap();
    state
        .lifecycle
        .vertex_oauth
        .get_owned(view["id"].as_str().unwrap(), &owner)
        .unwrap()
}
pub(super) async fn create(state: &Arc<AppState>, grant: &str, body: Value, mock: &Mock) -> Value {
    let response = test_endpoint(mock.endpoint.clone(), call(state, ROOT, body, Some(grant))).await;
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "{}",
        support::parse_json(response).await
    );
    support::parse_json(response).await["session"].clone()
}
pub(super) fn callback(view: &Value) -> String {
    let auth = url::Url::parse(view["authorizationUrl"].as_str().unwrap()).unwrap();
    let query: std::collections::HashMap<_, _> = auth.query_pairs().into_owned().collect();
    assert_eq!(query["scope"], CLOUD_PLATFORM_SCOPE);
    assert_eq!(query["code_challenge_method"], "S256");
    assert_eq!(query["redirect_uri"], view["redirectUri"].as_str().unwrap());
    format!(
        "{}?code=synthetic-code&state={}",
        query["redirect_uri"], query["state"]
    )
}
pub(super) async fn action(
    state: &Arc<AppState>,
    view: &Value,
    grant: Option<&str>,
    name: &str,
    callback: Option<&str>,
) -> axum::response::Response {
    let mut body = json!({"expectedRevision":view["expectedRevision"]});
    if let Some(callback) = callback {
        body["callbackUrl"] = json!(callback);
    }
    crate::protocol::vertex_oauth::transport_test_support::inherit_endpoint(call(
        state,
        &format!("{ROOT}/{}/{name}", view["id"].as_str().unwrap()),
        body,
        grant,
    ))
    .await
}
pub(super) async fn complete(state: &Arc<AppState>, view: &Value, grant: &str, mock: &Mock) {
    let response = test_endpoint(
        mock.endpoint.clone(),
        action(state, view, Some(grant), "complete", Some(&callback(view))),
    )
    .await;
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "{}",
        support::parse_json(response).await
    );
    let body = support::parse_json(response).await;
    assert_eq!(body["session"]["status"], "ready");
    assert_redacted(&body);
}
pub(super) fn assert_redacted(value: &Value) {
    let public = value.to_string();
    for secret in [
        "synthetic-new-access",
        "synthetic-new-refresh",
        "synthetic-private-client",
        "synthetic-other-secret",
    ] {
        assert!(!public.contains(secret), "secret in management view");
    }
}
pub(super) async fn callback_request(view: &Value) -> String {
    let url = url::Url::parse(&callback(view)).unwrap();
    format!(
        "GET /?{} HTTP/1.1\r\nHost: 127.0.0.1:{}\r\n\r\n",
        url.query().unwrap(),
        url.port().unwrap()
    )
}
pub(super) async fn connect(view: &Value) -> tokio::net::TcpStream {
    let url = url::Url::parse(view["redirectUri"].as_str().unwrap()).unwrap();
    tokio::net::TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, url.port().unwrap()))
        .await
        .unwrap()
}
pub(super) async fn deliver(view: &Value) -> String {
    let mut stream = connect(view).await;
    stream
        .write_all(callback_request(view).await.as_bytes())
        .await
        .unwrap();
    let mut reply = String::new();
    tokio::time::timeout(
        std::time::Duration::from_secs(3),
        stream.read_to_string(&mut reply),
    )
    .await
    .unwrap()
    .unwrap();
    reply
}
pub(super) async fn wait_status(
    session: &Session,
    status: crate::console::vertex_oauth::SessionStatus,
) {
    tokio::time::timeout(std::time::Duration::from_secs(4), async {
        while session.view().status != status {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
}
