use super::credential_payload::normalize_provider_credential_payload_for_storage;
use crate::db::GatewayProviderAccountView;
use crate::routing::candidate::ProviderExecutionMode;
use serde_json::{json, Value};

fn build_test_provider_account() -> GatewayProviderAccountView {
    GatewayProviderAccountView {
        id: "provider-accio-live".to_string(),
        label: "Accio Live".to_string(),
        service_provider_key: "accio_platform".to_string(),
        service_provider_label: "Accio".to_string(),
        adapter: "accio_compatible".to_string(),
        protocol_family: "openai_responses".to_string(),
        protocol_profile: "accio".to_string(),
        status: "active".to_string(),
        source_kind: Some("web_reverse_api".to_string()),
        aggregator_api_mode: None,
        web_reverse_access_mode: Some("direct_http_replay".to_string()),
        source_notes: Some(
            "Accio phoenix-gw live regression surface backed by imported manager accounts."
                .to_string(),
        ),
        execution_mode: ProviderExecutionMode::DirectHttp,
        endpoint_execution_modes: None,
        payload: json!({
            "adapter": "accio_compatible",
            "baseUrl": "https://phoenix-gw.alibaba.com",
            "defaultModel": "claude-opus-4-6",
        }),
        storage_mode: "inline".to_string(),
        cooldown_until: None,
        last_error: None,
        failure_count: 0,
        last_health_check_at: None,
        created_at: "2026-05-17T00:00:00Z".to_string(),
        updated_at: "2026-05-17T00:00:00Z".to_string(),
    }
}

#[test]
fn provider_credential_storage_normalization_drops_empty_top_level_overrides() {
    let provider = build_test_provider_account();
    let normalized = normalize_provider_credential_payload_for_storage(
        &provider,
        &json!({
            "baseUrl": "",
            "apiKey": "   ",
            "headers": {
                "Cookie": "cna=test-cna"
            }
        }),
    );
    let object = normalized
        .as_object()
        .expect("normalized provider credential payload should stay object");
    assert_eq!(
        object.get("adapter").and_then(Value::as_str),
        Some("accio_compatible")
    );
    assert!(
        !object.contains_key("baseUrl"),
        "empty baseUrl should not persist as credential-level override"
    );
    assert!(
        !object.contains_key("apiKey"),
        "empty apiKey should not persist as credential-level override"
    );
    let headers = object
        .get("headers")
        .and_then(Value::as_object)
        .expect("headers should remain an object");
    assert_eq!(
        headers.get("Cookie").and_then(Value::as_str),
        Some("cna=test-cna")
    );
}
