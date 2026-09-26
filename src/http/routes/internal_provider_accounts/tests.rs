use super::model_catalog::{
    extract_accio_catalog_model_names, filter_model_names_by_payload_constraints,
};
use super::model_discovery::deserialize_provider_payload_for_model_tiering;
use super::source_profile::infer_source_profile;
use crate::db;

#[test]
fn deserialize_provider_payload_for_model_tiering_backfills_missing_transport_fields() {
    let provider = db::GatewayProviderAccountView {
        id: "provider-without-auth".to_string(),
        label: "Provider Without Auth".to_string(),
        service_provider_key: "provider_without_auth".to_string(),
        service_provider_label: "Provider Without Auth".to_string(),
        adapter: "openai_compatible".to_string(),
        protocol_family: "openai".to_string(),
        protocol_profile: "openai".to_string(),
        status: "active".to_string(),
        source_kind: None,
        aggregator_api_mode: None,
        web_reverse_access_mode: None,
        source_notes: None,
        execution_mode: crate::routing::candidate::ProviderExecutionMode::DirectHttp,
        endpoint_execution_modes: None,
        payload: serde_json::json!({
            "defaultModel": "gpt-5.4-mini"
        }),
        storage_mode: "inline".to_string(),
        cooldown_until: None,
        last_error: None,
        failure_count: 0,
        last_health_check_at: None,
        created_at: "2026-04-20T00:00:00Z".to_string(),
        updated_at: "2026-04-20T00:00:00Z".to_string(),
    };

    let payload = deserialize_provider_payload_for_model_tiering(&provider)
        .expect("tiering payload should accept missing apiKey");

    assert_eq!(payload.adapter, "openai_compatible");
    assert_eq!(payload.base_url, "");
    assert_eq!(payload.api_key, "");
    assert_eq!(payload.default_model.as_deref(), Some("gpt-5.4-mini"));
}

#[test]
fn infer_codex_backend_as_official_vendor_api() {
    let provider = db::GatewayProviderAccountView {
        id: "codex-platform-provider".to_string(),
        label: "Codex Platform".to_string(),
        service_provider_key: "codex_platform".to_string(),
        service_provider_label: "Codex Platform".to_string(),
        adapter: "openai_compatible".to_string(),
        protocol_family: "openai".to_string(),
        protocol_profile: "codex".to_string(),
        status: "active".to_string(),
        source_kind: None,
        aggregator_api_mode: None,
        web_reverse_access_mode: None,
        source_notes: None,
        execution_mode: crate::routing::candidate::ProviderExecutionMode::DirectHttp,
        endpoint_execution_modes: None,
        payload: serde_json::json!({
            "adapter": "openai_compatible",
            "base_url": "https://chatgpt.com/backend-api/codex",
            "default_model": "gpt-5.4"
        }),
        storage_mode: "inline".to_string(),
        cooldown_until: None,
        last_error: None,
        failure_count: 0,
        last_health_check_at: None,
        created_at: "2026-04-16T00:00:00Z".to_string(),
        updated_at: "2026-04-16T00:00:00Z".to_string(),
    };

    let inferred = infer_source_profile(&provider);
    assert_eq!(inferred.source_kind, "official_vendor_api");
    assert_eq!(inferred.web_reverse_access_mode, None);
}

#[test]
fn infer_accio_as_web_reverse_direct_http_replay() {
    let provider = db::GatewayProviderAccountView {
        id: "accio-provider".to_string(),
        label: "Accio Live".to_string(),
        service_provider_key: "accio_platform".to_string(),
        service_provider_label: "Accio".to_string(),
        adapter: "accio_compatible".to_string(),
        protocol_family: "openai_responses".to_string(),
        protocol_profile: "accio".to_string(),
        status: "active".to_string(),
        source_kind: None,
        aggregator_api_mode: None,
        web_reverse_access_mode: None,
        source_notes: None,
        execution_mode: crate::routing::candidate::ProviderExecutionMode::DirectHttp,
        endpoint_execution_modes: None,
        payload: serde_json::json!({
            "adapter": "accio_compatible",
            "base_url": "https://phoenix-gw.alibaba.com",
            "default_model": "claude-sonnet-4-6"
        }),
        storage_mode: "inline".to_string(),
        cooldown_until: None,
        last_error: None,
        failure_count: 0,
        last_health_check_at: None,
        created_at: "2026-04-20T00:00:00Z".to_string(),
        updated_at: "2026-04-20T00:00:00Z".to_string(),
    };

    let inferred = infer_source_profile(&provider);
    assert_eq!(inferred.source_kind, "web_reverse_api");
    assert_eq!(
        inferred.web_reverse_access_mode.as_deref(),
        Some("direct_http_replay")
    );
}

#[test]
fn extracts_accio_catalog_model_list_and_filters_disabled_entries() {
    let raw_catalog = serde_json::json!({
        "success": true,
        "data": [
            {
                "provider": "claude",
                "modelList": [
                    {"modelName": "claude-sonnet-4-6", "visible": true},
                    {"modelName": "claude-opus-4-6", "visible": true}
                ]
            },
            {
                "provider": "gemini",
                "modelList": [
                    {"modelName": "gemini-3.1-pro-preview", "visible": true}
                ]
            }
        ]
    });
    assert_eq!(
        extract_accio_catalog_model_names(&raw_catalog),
        vec![
            "claude-opus-4-6".to_string(),
            "claude-sonnet-4-6".to_string(),
            "gemini-3.1-pro-preview".to_string()
        ]
    );

    let filtered = filter_model_names_by_payload_constraints(
        &serde_json::json!({
            "disabledModels": {
                "claude-opus-4-6": "quota_empty"
            }
        }),
        extract_accio_catalog_model_names(&raw_catalog),
    );
    assert_eq!(
        filtered,
        vec![
            "claude-sonnet-4-6".to_string(),
            "gemini-3.1-pro-preview".to_string()
        ]
    );
}
