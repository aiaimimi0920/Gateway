use super::*;
use serde_json::{json, Value};

fn payload() -> ProviderAccountPayload {
    serde_json::from_value(json!({"adapter":"kiro_compatible","base_url":BASE_URL,
        "api_key":"ksk_synthetic_only","auth_mode":AUTH_MODE,"responses_path":"/"}))
    .unwrap()
}
#[test]
fn kiro_api_key_material_is_bounded_without_inventing_a_complete_key_schema() {
    assert!(valid_material("ksk_synthetic_only"));
    assert!(valid_material("future-opaque-format"));
    for value in ["", "two words", "secret\r\nheader", "中文"] {
        assert!(!valid_material(value));
    }
    assert!(!valid_material(&"x".repeat(32_769)));
}
#[cfg(feature = "line-kiro-official-vendor-api")]
#[test]
fn kiro_api_key_rejects_ide_aliases_and_cross_product_transport_before_send() {
    validate(&payload(), "kiro_compatible").unwrap();
    for (name, value) in [
        ("session_auth", json!({"transport":"bearer"})),
        ("auth_token", json!("oauth")),
        ("auth_header_name", json!("authorization")),
        ("headers", json!({"Cookie":"private"})),
        ("extra_body", json!({"kiroMachineId":"private"})),
        ("execution_mode", json!("browser_backed")),
        ("base_url", json!("https://runtime.eu-central-1.kiro.dev")),
        (
            "base_url",
            json!("https://runtime.us-east-1.kiro.dev?query=1"),
        ),
        ("responses_path", json!("/generateAssistantResponse")),
        ("messages_path", json!("/messages")),
        ("call_flow", json!({"call_adapter":"openai"})),
        ("request_client_profile", json!("codex_cli")),
    ] {
        let mut raw = serde_json::to_value(payload()).unwrap();
        raw[name] = value;
        let bad = serde_json::from_value(raw).unwrap();
        assert!(
            crate::protocol::call_flow::prepare(&bad).is_err(),
            "accepted {name}"
        );
    }
}
#[cfg(feature = "line-kiro-official-vendor-api")]
#[test]
fn kiro_api_key_configuration_keeps_empty_template_and_static_credential_without_refresh() {
    use crate::routing::config::{RouteConfigStore, RouteConfigYaml};
    fn compile(provider: Value) -> anyhow::Result<RouteConfigStore> {
        let document: RouteConfigYaml =
            serde_json::from_value(json!({"providers":[provider]})).unwrap();
        RouteConfigStore::from_document(document)
    }
    let provider = json!({"id":"native-key","preset":PRESET_ID,"base_url":BASE_URL,
        "explicit_credentials_only":true,"credentials":[]});
    let store = compile(provider.clone()).unwrap();
    assert!(store.snapshot().get_providers()[0]
        .credential_pool
        .is_empty());
    let mut active = provider.clone();
    active["credentials"] = json!([{"id":"key-account","api_key":"ksk_synthetic_only","supported_models":["exact-model"]}]);
    let store = compile(active.clone()).unwrap();
    let compiled = &store.snapshot().get_providers()[0];
    assert_eq!(compiled.payload.auth_mode.as_deref(), Some(AUTH_MODE));
    assert_eq!(compiled.protocol_profile, "kiro");
    assert!(compiled.credential_pool[0].refresh_config.is_none());
    assert!(compiled.credential_pool[0].payload.default_model.is_none());
    for (name, value) in [
        ("refresh_token", json!("rt")),
        (
            "refresh_endpoint",
            json!("https://oidc.us-east-1.amazonaws.com/token"),
        ),
        ("refresh_client_id", json!("client")),
        ("token_expires_in_secs", json!(3600)),
        ("auth_mode", json!("bearer")),
        ("base_url", json!("https://q.us-east-1.amazonaws.com")),
    ] {
        let mut bad = active.clone();
        bad["credentials"][0][name] = value;
        assert!(compile(bad).is_err(), "accepted {name}");
    }
    let mut erased = active;
    erased["auth_mode"] = json!("bearer");
    erased["base_url"] = json!("https://proxy.invalid");
    assert!(compile(erased).is_err());
}
#[cfg(not(feature = "line-kiro-official-vendor-api"))]
#[test]
fn kiro_api_key_feature_off_refuses_runtime_even_with_explicit_marker() {
    assert_eq!(
        validate(&payload(), "kiro_compatible")
            .unwrap_err()
            .code
            .as_deref(),
        Some("gateway_provider_line_compiled_out")
    );
    assert!(crate::preset::get_builtin_preset(PRESET_ID).is_none());
}
