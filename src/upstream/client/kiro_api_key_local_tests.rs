//! Real TCP management and production local persistence; no fake backend or provider network.
use super::kiro_api_key_test_support as wire;
use crate::{access_balance::AccessBalanceStore, config::Config};
use serde_json::{json, Value};
use std::{future::Future, path::Path, time::Duration};
mod browser_fixture;
mod runtime;
#[path = "../../../tests/console_contract_support/mod.rs"]
mod support;
pub(super) use runtime::LocalServer;

const KEY: &str = "synthetic-local-kiro-key";
const MODEL: &str = "Exact.Model/Version";
const ROUTE: &str = "/v1/internal/gateway/console/route-config";
const KEY_PATH: &str = "/providers/0/credentials/0/api_key";

pub(super) fn run<F: Future<Output = ()> + 'static>(seconds: u64, test: fn() -> F) {
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async {
                    // Construct the bootstrap future inside its current-thread runtime, not across threads.
                    tokio::time::timeout(Duration::from_secs(seconds), test())
                        .await
                        .unwrap()
                });
        })
        .unwrap()
        .join()
        .unwrap();
}

pub(super) fn config(root: &Path) -> Config {
    std::fs::create_dir_all(root).unwrap();
    let mut config = support::test_config();
    config.storage_mode = crate::config::GatewayStorageMode::Local;
    config.gateway_api_key = None;
    config.gateway_api_key_secret = None;
    config.console.state_dir = root.join("state");
    config.console.routes_file = root.join("routes.yaml");
    config.console.management_ui_enabled = false;
    config.upstream_timeout_secs = 5;
    std::fs::write(
        &config.console.routes_file,
        "providers: []\nmodel_routes: []\n",
    )
    .unwrap();
    config
}

pub(super) async fn remaining(server: &LocalServer, id: &str) -> i64 {
    AccessBalanceStore::from_state(&server.state)
        .unwrap()
        .get(id)
        .await
        .unwrap()
        .unwrap()
        .remaining_messages
        .unwrap()
}

fn candidate(server: &LocalServer, document: Value, operation: &str) -> Value {
    let mut patch = json!({"path":KEY_PATH,"operation":operation});
    if operation == "replace" {
        patch["value"] = json!(KEY);
    }
    json!({"document":document,"secretPatches":[patch],"expectedRevision":server.revision()})
}

#[test]
fn kiro_api_key_local_tcp_secret_save_call_and_restart_preserve_identity_and_balance() {
    run(90, local_flow);
}

async fn local_flow() {
    let root = std::env::temp_dir().join(format!("kiro-key-local-{}", uuid::Uuid::new_v4()));
    let config = config(&root);
    let upstream = wire::Server::stream(wire::event_frame("local-answer"), false).await;
    let server = LocalServer::start(config.clone(), &upstream, None).await;
    let port = server.port;
    let grant = server.grant().await;
    let document = json!({"providers":[{"id":"kiro-api","preset":"kiro-api-key",
        "base_url":crate::protocol::kiro_api_key::BASE_URL,"explicit_credentials_only":true,
        "credentials":[{"id":"stable-local","api_key":null,"supported_models":[MODEL]}]}],"model_routes":[]});
    let original = candidate(&server, document, "replace");
    let (status, saved) = server
        .manage("PUT", ROUTE, original.clone(), Some(&grant))
        .await;
    assert_eq!(status, 200, "{saved}");
    assert!(!saved.to_string().contains(KEY));
    assert_eq!(
        server.manage("PUT", ROUTE, original, Some(&grant)).await.0,
        409
    );
    let created = server
        .manage(
            "POST",
            "/v1/internal/gateway/access/keys",
            json!({"ownerType":"user","ownerId":"local-fixture","resolvedProjectId":"local",
            "resolvedTenantId":"local","keyKind":"normal","publicKeyPrefix":"sk-gw",
            "displayName":"Kiro local flow","metadata":{"models":[MODEL]}}),
            None,
        )
        .await;
    assert_eq!(created.0, 200, "{}", created.1);
    let id = created.1["id"].as_str().unwrap();
    let token = created.1["token"].as_str().unwrap();
    let balance_path = format!("/v1/internal/gateway/access/keys/{id}/balances/adjust");
    assert_eq!(
        server
            .manage(
                "POST",
                &balance_path,
                json!({"balanceMode":"message_prepaid","totalMessages":4,"remainingMessages":4}),
                None
            )
            .await
            .0,
        200
    );
    let (status, text) = server.call(token, false).await;
    assert_eq!(status, 200, "{text}");
    assert!(text.contains("local-answer") && !text.contains(KEY));
    assert_eq!(remaining(&server, id).await, 3);

    let mut metadata = saved["routeConfig"]["document"].clone();
    metadata["providers"][0]["credentials"][0]["account_name"] = json!("kept-metadata");
    let updated = server
        .manage("PUT", ROUTE, candidate(&server, metadata, "keep"), None)
        .await;
    assert_eq!(updated.0, 200, "{}", updated.1);
    let revision = server.revision();
    server.stop().await;

    let server = LocalServer::start(config.clone(), &upstream, Some(port)).await;
    assert_eq!(server.revision(), revision);
    assert_eq!(remaining(&server, id).await, 3);
    let account = &server.state.route_config.get_providers()[0].credential_pool[0];
    assert_eq!(account.id, "stable-local");
    assert_eq!(account.payload.api_key, KEY);
    assert_eq!(account.payload.auth_mode.as_deref(), Some("kiro_api_key"));
    let clear = candidate(
        &server,
        updated.1["routeConfig"]["document"].clone(),
        "clear",
    );
    assert_eq!(
        server
            .manage("PUT", ROUTE, clear.clone(), Some(&grant))
            .await
            .0,
        403
    );
    let (status, text) = server.call(token, true).await;
    assert_eq!(status, 200, "{text}");
    assert!(text.contains("local-answer") && text.contains("[DONE]"));
    tokio::time::timeout(Duration::from_secs(3), async {
        while remaining(&server, id).await != 2 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let fresh_grant = server.grant().await;
    assert_eq!(
        server
            .manage("PUT", ROUTE, clear, Some(&fresh_grant))
            .await
            .0,
        200
    );
    assert_ne!(server.call(token, false).await.0, 200);
    assert_eq!(upstream.attempts(), 2);
    server.stop().await;

    let server = LocalServer::start(config, &upstream, Some(port)).await;
    assert!(
        server.state.route_config.get_providers()[0].credential_pool[0]
            .payload
            .api_key
            .is_empty()
    );
    assert_ne!(server.call(token, false).await.0, 200);
    assert_eq!(remaining(&server, id).await, 2);
    assert_eq!(upstream.attempts(), 2);
    assert_wire(&upstream);
    server.stop().await;
    upstream.stop().await;
}

fn assert_wire(upstream: &wire::Server) {
    let calls = upstream.calls.lock().unwrap();
    assert_eq!(calls.len(), 2);
    for (headers, body) in calls.iter() {
        assert_eq!(
            headers["authorization"].to_str().unwrap(),
            format!("Bearer {KEY}")
        );
        assert_eq!(headers["tokentype"], "API_KEY");
        assert_eq!(
            headers["x-amz-target"],
            crate::protocol::kiro_api_key::TARGET
        );
        let current = &body["conversationState"]["currentMessage"]["userInputMessage"];
        assert_eq!(current["modelId"], MODEL);
        assert_eq!(current["origin"], "KIRO_CLI");
        assert!(body.get("profileArn").is_none());
    }
}
