//! Synthetic material and pure SecretPatch transformations; no upstream or local state.
use neuro_gateway::console::secrets::{redact_route_document, resolve_secret_patches, SecretPatch};
use neuro_gateway::routing::config::RouteConfigYaml;
use serde_json::{json, Value};

pub const BASE: &str = "https://workspace-a.cn-beijing.maas.aliyuncs.com/compatible-mode/v1";
pub const OTHER: &str = "https://workspace-b.ap-southeast-1.maas.aliyuncs.com/compatible-mode/v1";

pub fn document(value: Value) -> RouteConfigYaml {
    serde_json::from_value(value).unwrap()
}

pub fn fixture(shared: bool) -> RouteConfigYaml {
    document(json!({"providers":[{
        "id":"arbitrary-name", "preset":"qwen-official-api", "auth_mode":"qwen_workspace_api",
        "base_url":BASE, "explicit_credentials_only":true,
        "api_key":if shared { "synthetic-provider-key" } else { "" },
        "credentials":[{"id":"a", "api_key":if shared { Value::Null } else { json!("synthetic-account-key") }}]
    }], "model_routes":[], "aliases":{}}))
}

pub fn redacted(active: &RouteConfigYaml) -> (Value, Vec<SecretPatch>) {
    let redacted = redact_route_document(active).unwrap();
    let keeps = redacted
        .secrets
        .iter()
        .filter(|s| s.configured)
        .map(|s| SecretPatch::keep(&s.path))
        .collect();
    (serde_json::to_value(redacted.document).unwrap(), keeps)
}

pub fn reject(active: &RouteConfigYaml, draft: Value, patches: &[SecretPatch]) {
    let before = serde_json::to_value(active).unwrap();
    let error = resolve_secret_patches(active, document(draft), patches)
        .expect_err("keep cannot change a protected secret's audience");
    assert_eq!(error.code(), "secret_keep_destination_changed");
    assert!(!error.to_string().contains("synthetic-"));
    assert!(!error.to_string().contains("maas.aliyuncs.com"));
    assert_eq!(serde_json::to_value(active).unwrap(), before);
}
