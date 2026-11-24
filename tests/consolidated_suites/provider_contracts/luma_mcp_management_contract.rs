//! Read-only MCP accounts are durable management data, never selectable inference credentials.
use neuro_gateway::routing::config::{RouteConfigStore, RouteConfigYaml};
use serde_json::{json, Value};

fn provider() -> Value {
    json!({"id":"luma-mcp","base_url":"https://agents.lumalabs.ai/mcp",
        "adapter":"lumalabs_mcp_readonly","protocol_profile":"lumalabs_mcp_readonly",
        "protocol_family":"lumalabs_mcp","auth_mode":"luma_mcp_oauth",
        "explicit_credentials_only":true,"credentials":[{
            "id":"saved-mcp","enabled":false,"api_key":"synthetic-access",
            "luma_oauth":{"issuer":"https://agents.lumalabs.ai",
                "resource":"https://agents.lumalabs.ai/mcp",
                "token_endpoint":"https://auth.lumalabs.ai/oauth2/token",
                "client_id":"registered-client","redirect_uri":"http://127.0.0.1:54421/callback"}}]})
}
fn document(provider: Value) -> RouteConfigYaml {
    serde_json::from_value(json!({"providers":[provider]})).unwrap()
}

#[test]
fn luma_mcp_saved_context_survives_document_roundtrip() {
    let document = document(provider());
    let encoded = serde_json::to_value(&document).unwrap();
    assert_eq!(
        encoded["providers"][0]["credentials"][0]["luma_oauth"],
        provider()["credentials"][0]["luma_oauth"]
    );
}

#[test]
fn luma_mcp_saved_account_compiles_only_as_disabled_management_data() {
    let store = RouteConfigStore::from_document(document(provider())).unwrap();
    let providers = store.get_providers();
    assert_eq!(providers[0].credential_pool.len(), 1);
    assert!(!providers[0].credential_pool[0].enabled);
    assert!(providers[0].credential_pool[0].refresh_config.is_none());
    let payload = &providers[0].credential_pool[0].payload;
    assert_eq!(
        neuro_gateway::protocol::call_flow::prepare(payload)
            .err()
            .unwrap()
            .code
            .as_deref(),
        Some("luma_mcp_management_only")
    );
}

#[test]
fn luma_mcp_config_cannot_mix_products_enable_calls_or_inherit_authorization() {
    for (pointer, value) in [
        ("/adapter", json!("openai_compatible")),
        ("/auth_mode", json!("bearer")),
        ("/protocol_profile", json!("lumalabs_agents_api")),
        ("/protocol_family", json!("openai_chat")),
        ("/base_url", json!("https://agents.lumalabs.ai/v1")),
        ("/explicit_credentials_only", json!(false)),
        ("/credentials/0/enabled", json!(true)),
        ("/credentials/0/enabled", Value::Null),
        ("/credentials/0/luma_oauth", Value::Null),
        ("/credentials/0/api_key", json!("luma-api-synthetic-key")),
    ] {
        let mut p = provider();
        *p.pointer_mut(pointer).unwrap() = value;
        assert!(
            RouteConfigStore::from_document(document(p)).is_err(),
            "{pointer}"
        );
    }
    for (key, value) in [
        ("preset", json!("lumalabs-api")),
        ("api_key", json!("parent-secret")),
        ("scheduled_probe_enabled", json!(true)),
        ("auto_refill_enabled", json!(true)),
        ("supported_models", json!(["model"])),
        ("balance_path", json!("https://other.invalid")),
        ("headers", json!({"Authorization":"other-secret"})),
        ("extra_body", json!({"private":"override"})),
        ("auth_token", json!("web-cookie")),
        (
            "call_flow",
            json!({"call_adapter":"auto","fingerprint_adapter":"auto","special_adapter":"auto"}),
        ),
    ] {
        let mut p = provider();
        p[key] = value;
        assert!(
            RouteConfigStore::from_document(document(p)).is_err(),
            "{key}"
        );
    }
    for (key, value) in [
        ("refresh_token", json!("refresh")),
        ("copilot_github_token", json!("other-product-secret")),
        ("refresh_endpoint", json!("https://other.invalid")),
        ("token_expires_in_secs", json!(3600)),
        ("auth_token", json!("cookie")),
        ("headers", json!({"Cookie":"other"})),
        ("scheduled_probe_enabled", json!(true)),
        ("base_url", json!("https://other.invalid")),
        ("supported_models", json!(["model"])),
    ] {
        let mut p = provider();
        p["credentials"][0][key] = value;
        assert!(
            RouteConfigStore::from_document(document(p)).is_err(),
            "{key}"
        );
    }
}

#[test]
fn luma_mcp_empty_template_expired_context_and_cleared_secret_remain_readable() {
    for change in 0..3 {
        let mut p = provider();
        match change {
            0 => p["credentials"] = json!([]),
            1 => p["credentials"][0]["api_key"] = Value::Null,
            _ => p["credentials"][0]["luma_oauth"]["expires_at"] = json!("2020-01-01T00:00:00Z"),
        }
        assert!(RouteConfigStore::from_document(document(p)).is_ok());
    }
}
