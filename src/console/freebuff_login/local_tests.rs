//! Normal management TCP, disposable Local SQLite, and a non-forwarding CLI approval peer.
use crate::{config::Config, routing::config::ProviderCredentialYaml};
use serde_json::{json, Value};
use std::{path::Path, time::Duration};
mod browser_fixture;
mod business;
mod flow;
mod peer;
mod runtime;
#[path = "../../../tests/console_contract_support/mod.rs"]
mod support;
use peer::Peer;
use runtime::LocalServer;

const PROVIDER: &str = "freebuff-local";
const ROUTE: &str = "/v1/internal/gateway/console/route-config";
const AUTH: &str = "/v1/internal/gateway/console/freebuff-auth-sessions";

fn config(root: &Path) -> Config {
    assert!(
        root.is_absolute() && !root.exists(),
        "Explicit new fixture root required"
    );
    std::fs::create_dir_all(root).unwrap();
    let mut config = support::test_config();
    config.storage_mode = crate::config::GatewayStorageMode::Local;
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
fn account(server: &LocalServer) -> ProviderCredentialYaml {
    let snapshot = server.state.route_config.snapshot();
    let provider = snapshot
        .document()
        .providers
        .iter()
        .find(|p| p.preset.as_deref() == Some("freebuff"))
        .unwrap();
    assert_eq!(provider.credentials.len(), 1);
    provider.credentials[0].clone()
}
fn assert_material(server: &LocalServer, generation: usize, device: &str) {
    let credential = account(server);
    assert_eq!(
        credential.api_key.as_deref(),
        Some(format!("synthetic-freebuff-token-{generation}").as_str())
    );
    let receipt = credential.freebuff_session.as_ref().unwrap();
    assert_eq!(receipt.fingerprint_id(), Some(device));
    assert_eq!(
        receipt.fingerprint_hash(),
        Some(format!("synthetic-hash-{generation}+/=").as_str())
    );
    assert!(
        credential.refresh_token.is_none()
            && credential.expires_at.is_none()
            && credential.auth_token.is_none()
    );
}
fn assert_public(body: &Value, peer: &Peer) {
    let text = body.to_string();
    for secret in [
        "synthetic-freebuff-token-",
        "synthetic-hash-",
        "synthetic-user@",
        "fingerprintId",
        "fingerprintHash",
    ] {
        assert!(
            !text.contains(secret),
            "Public management response contains {secret}"
        );
    }
    for device in peer.devices() {
        assert!(
            !text.contains(&device),
            "Public response leaked device receipt"
        );
    }
}
fn keep_document(server: &LocalServer, document: Value) -> Value {
    let patches: Vec<_> = [
        "api_key",
        "freebuff_session/fingerprint_id",
        "freebuff_session/fingerprint_hash",
    ]
    .iter()
    .map(|field| json!({"path":format!("/providers/0/credentials/0/{field}"),"operation":"keep"}))
    .collect();
    json!({"document":document,"secretPatches":patches,"expectedRevision":server.revision()})
}
