//! Production local bootstrap, real management TCP and distinct Console/SQLite persistence.
#![cfg(feature = "line-lumalabs-agents-official-api")]
#[path = "luma_agents_local_contract/browser_fixture.rs"]
mod browser_fixture;
#[path = "luma_agents_local_contract/runtime.rs"]
mod runtime;
#[path = "console_contract_support/mod.rs"]
mod support;
use neuro_gateway::{access_balance::AccessBalanceStore, config::Config};
use runtime::LocalServer;
use serde_json::{json, Value};
use std::{path::Path, time::Duration};

const KEY: &str = "luma-api-synthetic-local-only";
const ROUTE: &str = "/v1/internal/gateway/console/route-config";
const KEY_PATH: &str = "/providers/0/credentials/0/api_key";

fn config(root: &Path) -> Config {
    assert!(
        root.is_absolute() && !root.exists(),
        "Use a new absolute fixture root"
    );
    std::fs::create_dir_all(root).unwrap();
    let mut config = support::test_config();
    config.storage_mode = neuro_gateway::config::GatewayStorageMode::Local;
    config.gateway_api_key = None;
    config.gateway_api_key_secret = None;
    config.console.state_dir = root.join("state");
    config.console.routes_file = root.join("routes.yaml");
    config.console.management_ui_enabled = false;
    std::fs::write(
        &config.console.routes_file,
        "providers: []\nmodel_routes: []\n",
    )
    .unwrap();
    config
}
fn candidate(server: &LocalServer, document: Value, operation: &str) -> Value {
    let mut patch = json!({"path":KEY_PATH,"operation":operation});
    if operation == "replace" {
        patch["value"] = json!(KEY);
    }
    json!({"document":document,"secretPatches":[patch],"expectedRevision":server.revision()})
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

#[tokio::test]
async fn luma_agents_local_tcp_preserves_identity_keep_clear_and_sqlite_without_generation() {
    // Box the same-task bootstrap future, not a larger-stack test thread.
    tokio::time::timeout(Duration::from_secs(90), Box::pin(local_flow()))
        .await
        .unwrap();
}
async fn local_flow() {
    let root = std::env::temp_dir().join(format!("luma-local-{}", uuid::Uuid::new_v4()));
    let config = config(&root);
    let server = LocalServer::start(config.clone(), None).await;
    let port = server.port;
    let initial = server.revision();
    let grant = server.grant().await;
    let document = json!({"providers":[{"id":"renamed-luma","preset":"lumalabs-api",
        "base_url":"https://agents.lumalabs.ai/v1","explicit_credentials_only":true,
        "credentials":[{"id":"stable-local","api_key":null,"supported_models":["uni-1"]}]}],"model_routes":[]});
    let original = candidate(&server, document, "replace");
    assert_eq!(
        server.manage("PUT", ROUTE, original.clone(), None).await.0,
        403
    );
    assert_eq!(
        server
            .manage_with_origin(
                "PUT",
                ROUTE,
                original.clone(),
                Some(&grant),
                "https://wrong.invalid"
            )
            .await
            .0,
        403
    );
    assert_eq!(server.revision(), initial);
    let (status, saved) = server
        .manage("PUT", ROUTE, original.clone(), Some(&grant))
        .await;
    assert_eq!(status, 200, "{saved}");
    assert!(!saved.to_string().contains(KEY));
    assert_eq!(
        server.manage("PUT", ROUTE, original, Some(&grant)).await.0,
        409
    );
    let mut document = saved["routeConfig"]["document"].clone();
    document["providers"][0]["credentials"][0]["account_name"] = json!("metadata-kept");
    document["providers"][0]["credentials"][0]["supported_models"] = json!(["uni-1", "ray-3.2"]);
    let (status, kept) = server
        .manage("PUT", ROUTE, candidate(&server, document, "keep"), None)
        .await;
    assert_eq!(status, 200, "{kept}");
    let created = server.manage("POST", "/v1/internal/gateway/access/keys",
        json!({"ownerType":"user","ownerId":"local-luma-fixture","resolvedProjectId":"local","resolvedTenantId":"local",
        "keyKind":"normal","publicKeyPrefix":"sk-gw","displayName":"Luma persistence only","metadata":{"models":["uni-1"]}}), None).await;
    assert_eq!(created.0, 200, "{}", created.1);
    let id = created.1["id"].as_str().unwrap();
    let token = created.1["token"].as_str().unwrap().to_owned();
    let adjusted = server
        .manage(
            "POST",
            &format!("/v1/internal/gateway/access/keys/{id}/balances/adjust"),
            json!({"balanceMode":"message_prepaid","totalMessages":4,"remainingMessages":4}),
            None,
        )
        .await;
    assert_eq!(adjusted.0, 200, "{}", adjusted.1);
    let revision = server.revision();
    server.assert_account("renamed-luma", "stable-local", true);
    assert_eq!(remaining(&server, id).await, 4);
    server.stop().await;

    let server = LocalServer::start(config.clone(), Some(port)).await;
    assert_eq!(server.revision(), revision);
    server.assert_account("renamed-luma", "stable-local", true);
    assert_eq!(remaining(&server, id).await, 4);
    let copied = server
        .manage(
            "POST",
            &format!("/v1/internal/gateway/access/keys/{id}/secret"),
            json!(null),
            None,
        )
        .await;
    assert_eq!(copied.0, 200, "{}", copied.1);
    assert_eq!(copied.1["token"], token);
    let mut erased = kept["routeConfig"]["document"].clone();
    erased["providers"][0]["preset"] = json!("openai");
    erased["providers"][0]["base_url"] = json!("https://wrong.invalid/v1");
    let rejected = server
        .manage("PUT", ROUTE, candidate(&server, erased, "keep"), None)
        .await;
    assert_eq!(rejected.0, 400, "{}", rejected.1);
    assert_eq!(
        rejected.1["error"]["code"],
        "secret_keep_destination_changed"
    );
    assert_eq!(server.revision(), revision);
    let clear = candidate(&server, kept["routeConfig"]["document"].clone(), "clear");
    assert_eq!(
        server
            .manage("PUT", ROUTE, clear.clone(), Some(&grant))
            .await
            .0,
        403
    );
    let fresh = server.grant().await;
    let cleared = server.manage("PUT", ROUTE, clear, Some(&fresh)).await;
    assert_eq!(cleared.0, 200, "{}", cleared.1);
    assert!(!cleared.1.to_string().contains(KEY));
    let revision = server.revision();
    server.stop().await;

    let server = LocalServer::start(config, Some(port)).await;
    assert_eq!(server.revision(), revision);
    server.assert_account("renamed-luma", "stable-local", false);
    assert_eq!(remaining(&server, id).await, 4);
    assert!(root.join("state/runtime.sqlite3").is_file());
    server.stop().await;
}
