//! Production local bootstrap and normal secret/CAS management over real loopback TCP.
use super::vertex_express_test_support as wire;
use crate::{access_balance::AccessBalanceStore, config::Config};
use serde_json::{json, Value};
use std::{path::Path, time::Duration};
mod browser_fixture;
mod runtime;
#[path = "../../../tests/console_contract_support/mod.rs"]
mod support;
use runtime::LocalServer;

const KEY: &str = "synthetic-local-express-key";
const MODEL: &str = "gemini-contract";
const ROUTE: &str = "/v1/internal/gateway/console/route-config";
const KEY_PATH: &str = "/providers/0/credentials/0/api_key";

fn config(root: &Path) -> Config {
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

fn upstream_body() -> Vec<u8> {
    format!("data: {}\n\n", json!({"candidates":[{"index":0,"content":{"parts":[{"text":"local-answer"}]},"finishReason":"STOP"}],
        "usageMetadata":{"promptTokenCount":2,"candidatesTokenCount":3,"totalTokenCount":5}})).into_bytes()
}
async fn remaining(server: &LocalServer, id: &str) -> i64 {
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

#[tokio::test]
async fn vertex_express_local_tcp_save_call_and_rebuild_preserve_key_identity_and_balance() {
    // Same-task boxing and the default test stack; never copy an enlarged-stack fixture runner.
    tokio::time::timeout(Duration::from_secs(90), Box::pin(local_flow()))
        .await
        .unwrap();
}
async fn local_flow() {
    let root = std::env::temp_dir().join(format!("vertex-express-local-{}", uuid::Uuid::new_v4()));
    let config = config(&root);
    let upstream = wire::Server::start(200, upstream_body()).await;
    let server = LocalServer::start(config.clone(), &upstream, None).await;
    let port = server.port;
    let grant = server.grant().await;
    let document = json!({"providers":[{"id":"renamed-express","preset":"vertex-express-api",
        "base_url":crate::protocol::vertex_express::BASE_URL,"explicit_credentials_only":true,
        "credentials":[{"id":"stable-local","api_key":null,"supported_models":[MODEL]}]}],"model_routes":[]});
    let original = candidate(&server, document, "replace");
    assert_eq!(
        server.manage("PUT", ROUTE, original.clone(), None).await.0,
        403
    );
    let (status, saved) = server
        .manage("PUT", ROUTE, original.clone(), Some(&grant))
        .await;
    assert_eq!(status, 200, "{saved}");
    assert!(!saved.to_string().contains(KEY));
    assert_eq!(
        server.manage("PUT", ROUTE, original, Some(&grant)).await.0,
        409
    );
    let created = server.manage("POST", "/v1/internal/gateway/access/keys",
        json!({"ownerType":"user","ownerId":"local-fixture","resolvedProjectId":"local","resolvedTenantId":"local",
        "keyKind":"normal","publicKeyPrefix":"sk-gw","displayName":"Express local flow","metadata":{"models":[MODEL]}}), None).await;
    assert_eq!(created.0, 200, "{}", created.1);
    let id = created.1["id"].as_str().unwrap();
    let token = created.1["token"].as_str().unwrap();
    assert_eq!(
        server
            .manage(
                "POST",
                &format!("/v1/internal/gateway/access/keys/{id}/balances/adjust"),
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
    metadata["providers"][0]["credentials"][0]["supported_models"] =
        json!([MODEL, "gemini-metadata"]);
    let updated = server
        .manage("PUT", ROUTE, candidate(&server, metadata, "keep"), None)
        .await;
    assert_eq!(updated.0, 200, "{}", updated.1);
    let revision = server.revision();
    server.stop().await;

    let server = LocalServer::start(config.clone(), &upstream, Some(port)).await;
    assert_eq!(server.revision(), revision);
    assert_eq!(remaining(&server, id).await, 3);
    let providers = server.state.route_config.get_providers();
    let account = &providers[0].credential_pool[0];
    assert_eq!(account.id, "stable-local");
    assert_eq!(account.payload.api_key, KEY);
    assert_eq!(
        account.payload.auth_mode.as_deref(),
        Some("vertex_express_api_key")
    );
    assert_eq!(
        account.payload.account_name.as_deref(),
        Some("kept-metadata")
    );
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
    server.stop().await;

    // Closing awaits the real local finalizer. Check durable settlement after a new bootstrap, not desired-value polling.
    let server = LocalServer::start(config.clone(), &upstream, Some(port)).await;
    assert_eq!(server.revision(), revision);
    assert_eq!(remaining(&server, id).await, 2);
    let fresh_grant = server.grant().await;
    assert_eq!(
        server
            .manage("PUT", ROUTE, clear, Some(&fresh_grant))
            .await
            .0,
        200
    );
    assert_eq!(server.call(token, false).await.0, 400);
    let cleared_revision = server.revision();
    server.stop().await;

    let server = LocalServer::start(config, &upstream, Some(port)).await;
    assert_eq!(server.revision(), cleared_revision);
    assert!(
        server.state.route_config.get_providers()[0].credential_pool[0]
            .payload
            .api_key
            .is_empty()
    );
    assert_eq!(server.call(token, false).await.0, 400);
    assert_eq!(remaining(&server, id).await, 2);
    assert_wire(&upstream);
    server.stop().await;
    upstream.stop().await;
}
fn assert_wire(upstream: &wire::Server) {
    let calls = upstream.calls.lock().unwrap();
    assert_eq!(
        calls.len(),
        2,
        "Only the two explicit generations may reach the upstream"
    );
    for (path, headers, body) in calls.iter() {
        assert_eq!(
            path,
            "/v1/publishers/google/models/gemini-contract:streamGenerateContent?alt=sse"
        );
        assert_eq!(headers["x-goog-api-key"], KEY);
        for name in ["authorization", "cookie", "x-goog-user-project"] {
            assert!(!headers.contains_key(name));
        }
        assert_eq!(body["contents"][0]["parts"][0]["text"], "local question");
    }
}
