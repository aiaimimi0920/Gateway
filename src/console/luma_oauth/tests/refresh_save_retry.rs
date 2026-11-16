//! A failed disk write retains the received successor; resume never consumes the grant again.
use super::{fixture::Peer, refresh_fixture::Local, support};
use crate::{console::RouteConfigPersistence, protocol::luma_oauth::test_support::ENDPOINT};
use axum::http::{Method, StatusCode};
use serde_json::{json, Value};
use std::time::Duration;

async fn status(state: &std::sync::Arc<crate::state::AppState>, path: &str, expected: &str) {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let response =
                support::send_console_json(state, Method::GET, path, Value::Null, None, None).await;
            let body = support::parse_json(response).await;
            if body["session"]["status"] == expected {
                break;
            }
            assert_eq!(body["session"]["status"], "importing", "{body}");
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn luma_explicit_refresh_save_failure_keeps_receipt_and_resume_never_resends() {
    let f = Local::new();
    let peer = Peer::with_reply(Duration::from_millis(300), json!({"access_token":"saved-after-retry", "token_type":"Bearer", "refresh_token":"rotated-after-retry"})).await;
    ENDPOINT.scope(peer.endpoint.clone(), async {
        let grant = support::grant_console_secret_access(&f.state).await;
        let revision = f.state.route_config.snapshot().revision().id().to_string();
        let root = "/v1/internal/gateway/console/luma-refresh-sessions";
        let response = support::send_console_json(&f.state, Method::POST, root,
            json!({"providerId":"luma-mcp","credentialId":"saved-mcp","expectedRevision":revision}), Some(&grant), None).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = support::parse_json(response).await;
        let id = body["session"]["id"].as_str().unwrap();
        let path = format!("{root}/{id}");
        let response = support::send_console_json(&f.state, Method::POST, &format!("{path}/import"), json!({"expectedRevision":revision}), Some(&grant), None).await;
        assert_eq!(response.status(), StatusCode::OK);
        peer.wait_call().await;
        let store = RouteConfigPersistence::new(&f.config.state_dir, &f.config.routes_file).unwrap();
        let guard = store.try_writer_lock().unwrap();
        status(&f.state, &path, "save_unconfirmed").await;
        assert_eq!(peer.count(), 1);
        assert_eq!(f.state.route_config.snapshot().revision().id(), revision);
        drop(guard);
        let response = support::send_console_json(&f.state, Method::POST, &format!("{path}/resume"), json!({"expectedRevision":revision}), Some(&grant), None).await;
        assert_eq!(response.status(), StatusCode::OK);
        status(&f.state, &path, "succeeded").await;
        assert_eq!(peer.count(), 1);
        assert_eq!(f.state.route_config.snapshot().document().providers[0].credentials[0].refresh_token.as_deref(), Some("rotated-after-retry"));
        f.state.lifecycle.luma_oauth.join().await;
    }).await;
    peer.stop().await;
}

#[tokio::test]
async fn luma_explicit_refresh_prepare_cancel_can_prepare_again_without_sending() {
    let f = Local::new();
    let registry = &f.state.lifecycle.luma_oauth;
    let revision = f.state.route_config.snapshot().revision().id().to_string();
    let first = registry
        .prepare_refresh(&f.state, "owner", "luma-mcp", "saved-mcp", &revision)
        .unwrap();
    registry.get_owned(&first.id, "owner").unwrap().cancel();
    let next = registry
        .prepare_refresh(&f.state, "owner", "luma-mcp", "saved-mcp", &revision)
        .unwrap();
    assert_ne!(first.id, next.id);
    assert!(registry.get_owned(&next.id, "other-owner").is_err());
    registry.join().await;
}
