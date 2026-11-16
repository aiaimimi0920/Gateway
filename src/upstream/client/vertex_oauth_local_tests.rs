//! Full Vertex normal management, real TCP/local SQLite, and non-forwarding synthetic peers.
use super::vertex_oauth_test_support as wire;
use crate::{config::Config, routing::config::ProviderCredentialYaml};
use serde_json::{json, Value};
use std::{path::Path, time::Duration};
mod browser_fixture;
mod flow;
mod issuer;
mod runtime;
#[path = "../../../tests/console_contract_support/mod.rs"]
mod support;
use issuer::Issuer;
use runtime::LocalServer;

const MODEL: &str = "gemini-contract";
const PROVIDER: &str = "vertex-oauth";
const CLIENT: &str = "synthetic-local-client";
const SECRET: &str = "synthetic-local-client-secret";
const REFRESHED_ACCESS: &str = "synthetic-refreshed-access";
const REFRESHED_TOKEN: &str = "synthetic-refreshed-token";
const BASE: &str =
    "https://aiplatform.googleapis.com/v1/projects/project-a/locations/global/publishers/google";
const ROUTE: &str = "/v1/internal/gateway/console/route-config";
const AUTH: &str = "/v1/internal/gateway/console/vertex-auth-sessions";

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
    format!("data: {}\n\n", json!({"candidates":[{"index":0,"content":{"parts":[{"text":"local-vertex-answer"}]},"finishReason":"STOP"}],
        "usageMetadata":{"promptTokenCount":2,"candidatesTokenCount":3,"totalTokenCount":5}})).into_bytes()
}
fn target() -> Value {
    json!({"project":"project-a","location":"global","quota_project":"quota-a"})
}
fn account(server: &LocalServer) -> ProviderCredentialYaml {
    server.state.route_config.snapshot().document().providers[0].credentials[0].clone()
}
fn keep_document(server: &LocalServer, document: Value) -> Value {
    let secret_patches: Vec<_> = ["api_key", "refresh_token", "vertex_oauth/client_secret"].iter()
        .map(|field| json!({"path":format!("/providers/0/credentials/0/{field}"),"operation":"keep"})).collect();
    json!({"document":document,"secretPatches":secret_patches,"expectedRevision":server.revision()})
}
async fn wait_access(server: &LocalServer, expected: &str) {
    tokio::time::timeout(Duration::from_secs(10), async {
        while account(server).api_key.as_deref() != Some(expected) {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
}
fn assert_wire(upstream: &wire::Server, expected: &[&str]) {
    let calls = upstream.calls.lock().unwrap();
    assert_eq!(calls.len(), expected.len());
    for ((path, headers, body), access) in calls.iter().zip(expected) {
        assert_eq!(path, "/v1/projects/project-a/locations/global/publishers/google/models/gemini-contract:streamGenerateContent?alt=sse");
        assert_eq!(headers["authorization"], format!("Bearer {access}"));
        assert_eq!(headers["x-goog-user-project"], "quota-a");
        assert!(!headers.contains_key("x-goog-api-key") && !headers.contains_key("cookie"));
        assert_eq!(body["contents"][0]["parts"][0]["text"], "local question");
    }
}
