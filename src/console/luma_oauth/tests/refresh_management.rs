//! Exercise the normal management entry, runtime owner and persisted account with a synthetic peer.
use super::{fixture::Peer, refresh_fixture::Local, support};
use crate::protocol::luma_oauth::test_support::ENDPOINT;
use axum::http::{Method, StatusCode};
use serde_json::{json, Value};
use std::time::Duration;

const PATH: &str = "/v1/internal/gateway/console/luma-refresh-sessions";

#[tokio::test]
async fn luma_explicit_refresh_management_rotates_original_account_once() {
    let f = Local::new();
    let peer = Peer::with_reply(
        Duration::from_millis(150),
        json!({
            "access_token":"rotated-access","refresh_token":"rotated-refresh",
            "token_type":"Bearer","expires_in":3600
        }),
    )
    .await;
    ENDPOINT.scope(peer.endpoint.clone(), async {
        let revision = f.state.route_config.snapshot().revision().id().to_string();
        let input = json!({"providerId":"luma-mcp","credentialId":"saved-mcp","expectedRevision":revision});
        let denied = support::send_console_json(&f.state, Method::POST, PATH, input.clone(), None, None).await;
        assert_eq!(denied.status(), StatusCode::FORBIDDEN);
        let grant = support::grant_console_secret_access(&f.state).await;
        let response = support::send_console_json(&f.state, Method::POST, PATH, input.clone(), Some(&grant), None).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()["cache-control"], "no-store");
        let first = support::parse_json(response).await["session"].clone();
        let duplicate = support::send_console_json(&f.state, Method::POST, PATH, input, Some(&grant), None).await;
        assert_eq!(duplicate.status(), StatusCode::OK);
        assert_eq!(support::parse_json(duplicate).await["session"]["id"], first["id"]);
        let path = format!("{PATH}/{}", first["id"].as_str().unwrap());
        assert_eq!(first["status"], "ready");
        assert_eq!(peer.count(), 0); // Preparing the confirmation does not consume the grant.
        for _ in 0..2 {
            let started = support::send_console_json(&f.state, Method::POST, &format!("{path}/import"),
                json!({"expectedRevision": revision}), Some(&grant), None).await;
            assert_eq!(started.status(), StatusCode::OK);
        }
        tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                let response = support::send_console_json(&f.state, Method::GET, &path, Value::Null, None, None).await;
                assert_eq!(response.status(), StatusCode::OK);
                let body = support::parse_json(response).await;
                assert!(!body.to_string().contains("rotated-"));
                if body["session"]["status"] == "succeeded" { break; }
                assert_eq!(body["session"]["status"], "importing", "{body}");
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }).await.unwrap();
        let snapshot = f.state.route_config.snapshot();
        let credential = &snapshot.document().providers[0].credentials[0];
        assert_eq!(credential.id.as_deref(), Some("saved-mcp"));
        assert_eq!(credential.enabled, Some(false));
        assert_eq!(credential.api_key.as_deref(), Some("rotated-access"));
        assert_eq!(credential.refresh_token.as_deref(), Some("rotated-refresh"));
        assert!(credential.expires_at.is_none());
        assert!(credential.token_expires_in_secs.is_none());
        assert!(credential.luma_oauth.as_ref().unwrap().expires_at().is_some());
        assert_ne!(snapshot.revision().id(), revision);
        assert_eq!(peer.count(), 1);
        f.state.lifecycle.luma_oauth.join().await;
        let reopened = f.reopen();
        let runtime = reopened.route_config_runtime.as_ref().unwrap();
        runtime.recover_refresh_authority().await.unwrap();
        runtime.repair_refresh_cache().await.unwrap();
        assert_eq!(reopened.route_config.snapshot().revision().id(), snapshot.revision().id());
        assert_eq!(reopened.route_config.snapshot().document().providers[0].credentials[0].refresh_token.as_deref(), Some("rotated-refresh"));
    }).await;
    peer.stop().await;
}
