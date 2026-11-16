//! Admission limits and joined cleanup use actual loopback sockets without Google or credentials.
use super::*;
use crate::routing::config::RouteConfigYaml;
use serde_json::json;
use tokio::io::AsyncWriteExt;
#[path = "../../../../tests/console_contract_support/mod.rs"]
mod support;

fn document() -> RouteConfigYaml {
    serde_json::from_value(json!({"providers":[{"id":"vertex","preset":"google-agent-platform-official-api",
        "auth_mode":"vertex_user_oauth","explicit_credentials_only":true,
        "base_url":"https://aiplatform.googleapis.com/v1/projects/project-a/locations/global/publishers/google","credentials":[]}]})).unwrap()
}
async fn create(
    state: &Arc<AppState>,
    ttl: Duration,
) -> Result<Arc<Session>, crate::error::GatewayError> {
    state
        .lifecycle
        .vertex_oauth
        .create_with_ttl(
            state,
            Binding::new(
                "resource-test-owner",
                "vertex",
                state.route_config.snapshot().revision().id(),
            )
            .unwrap(),
            OAuthClient::new("explicit-client", None).unwrap(),
            VertexTarget::new("project-a", "global", None).unwrap(),
            rquest::Client::builder().no_proxy().build().unwrap(),
            ttl,
        )
        .await
}
async fn connect(session: &Session) -> tokio::net::TcpStream {
    let url = url::Url::parse(&session.view().redirect_uri).unwrap();
    tokio::net::TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, url.port().unwrap()))
        .await
        .unwrap()
}
async fn assert_closed(session: &Session) {
    let url = url::Url::parse(&session.view().redirect_uri).unwrap();
    assert!(
        tokio::net::TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, url.port().unwrap()))
            .await
            .is_err()
    );
}

#[cfg(feature = "line-google-agent-platform-official")]
#[tokio::test]
async fn vertex_oauth_management_session_capacity_is_runtime_scoped_and_reclaimed_after_cancel() {
    let fixture = support::ConsoleStateFixture::new(document(), true);
    let registry = &fixture.state.lifecycle.vertex_oauth;
    let mut sessions = Vec::new();
    for _ in 0..16 {
        sessions.push(
            create(&fixture.state, Duration::from_secs(600))
                .await
                .unwrap(),
        );
    }
    assert!(create(&fixture.state, Duration::from_secs(600))
        .await
        .unwrap_err()
        .message
        .contains("capacity"));
    sessions[0].cancel();
    let replacement = create(&fixture.state, Duration::from_secs(600))
        .await
        .unwrap();
    assert_eq!(registry.sessions.lock().len(), 16);
    let other = support::ConsoleStateFixture::new(document(), true);
    create(&other.state, Duration::from_secs(600))
        .await
        .unwrap();
    other.state.lifecycle.vertex_oauth.join().await;
    tokio::time::timeout(Duration::from_secs(2), registry.join())
        .await
        .unwrap();
    for session in sessions {
        assert_closed(&session).await;
    }
    assert_closed(&replacement).await;
    assert!(registry.operations.lock().is_empty());
}

#[cfg(feature = "line-google-agent-platform-official")]
#[tokio::test]
async fn vertex_oauth_management_operation_ceiling_keeps_accepted_import_query_idempotent() {
    let fixture = support::ConsoleStateFixture::new(document(), true);
    let registry = &fixture.state.lifecycle.vertex_oauth;
    let session = create(&fixture.state, Duration::from_secs(600))
        .await
        .unwrap();
    {
        let mut operations = registry.operations.lock();
        while operations.len() < 32 {
            operations.spawn(std::future::pending());
        }
    }
    assert!(create(&fixture.state, Duration::from_secs(600))
        .await
        .unwrap_err()
        .message
        .contains("capacity"));
    assert!(registry
        .start_import(fixture.state.clone(), session.id(), session.binding())
        .err()
        .unwrap()
        .message
        .contains("capacity"));
    session.inner.lock().status = super::super::SessionStatus::Importing;
    assert_eq!(
        registry
            .start_import(fixture.state.clone(), session.id(), session.binding())
            .unwrap()
            .status,
        super::super::SessionStatus::Importing
    );
    registry.operations.lock().abort_all();
    tokio::time::timeout(Duration::from_secs(2), registry.join())
        .await
        .unwrap();
}

#[cfg(feature = "line-google-agent-platform-official")]
#[tokio::test]
async fn vertex_oauth_management_expiry_closes_partial_header_and_worker_shutdown_joins_listeners()
{
    let fixture = support::ConsoleStateFixture::new(document(), true);
    let state = &fixture.state;
    let registry = &state.lifecycle.vertex_oauth;
    let expired = create(state, Duration::from_millis(50)).await.unwrap();
    let mut partial = connect(&expired).await;
    partial
        .write_all(b"GET /?code=c HTTP/1.1\r\nHost: ")
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(expired.view().status, super::super::SessionStatus::Expired);
    let waiting = create(state, Duration::from_secs(600)).await.unwrap();
    let mut partial = connect(&waiting).await;
    partial
        .write_all(b"GET /?code=c HTTP/1.1\r\nHost: ")
        .await
        .unwrap();
    registry.spawn(state.shutdown.clone());
    state.shutdown.request("synthetic_shutdown");
    tokio::time::timeout(Duration::from_secs(2), registry.join())
        .await
        .unwrap();
    assert_eq!(
        waiting.view().status,
        super::super::SessionStatus::Cancelled
    );
    assert_closed(&expired).await;
    assert_closed(&waiting).await;
    assert!(registry.operations.lock().is_empty());
    assert!(registry.worker.lock().is_none());
    assert!(create(state, Duration::from_secs(600)).await.is_err());
}

#[cfg(not(feature = "line-google-agent-platform-official"))]
#[tokio::test]
async fn vertex_oauth_management_feature_off_rejects_before_listener_or_transport() {
    let fixture = support::ConsoleStateFixture::new(support::document("vertex", "model", ""), true);
    let error = create(&fixture.state, Duration::from_secs(600))
        .await
        .unwrap_err();
    assert_eq!(error.code.as_deref(), Some("vertex_login_rejected"));
    assert!(error.message.contains("feature_disabled"));
    assert!(fixture
        .state
        .lifecycle
        .vertex_oauth
        .sessions
        .lock()
        .is_empty());
    assert!(fixture
        .state
        .lifecycle
        .vertex_oauth
        .operations
        .lock()
        .is_empty());
    fixture.state.lifecycle.vertex_oauth.join().await;
}
