//! Shared account/credential fixtures and folder-sync contract suites.
use crate::db;
use crate::routing::candidate::ProviderExecutionMode;

mod catalog_media_search;
mod catalog_text;
mod classification;
mod filesystem;
mod layout;
mod normalization_canvas;
mod normalization_chatgpt;
mod normalization_web;

fn build_test_provider_account(
    label: &str,
    adapter: &str,
    protocol_family: &str,
    protocol_profile: &str,
    base_url: &str,
    source_kind: Option<&str>,
    web_reverse_access_mode: Option<&str>,
) -> db::GatewayProviderAccountView {
    db::GatewayProviderAccountView {
        id: "provider-1".to_string(),
        label: label.to_string(),
        service_provider_key: "qwen_platform".to_string(),
        service_provider_label: "Qwen Platform".to_string(),
        adapter: adapter.to_string(),
        protocol_family: protocol_family.to_string(),
        protocol_profile: protocol_profile.to_string(),
        status: "active".to_string(),
        source_kind: source_kind.map(|value| value.to_string()),
        aggregator_api_mode: None,
        web_reverse_access_mode: web_reverse_access_mode.map(|value| value.to_string()),
        source_notes: None,
        execution_mode: ProviderExecutionMode::DirectHttp,
        endpoint_execution_modes: None,
        payload: serde_json::json!({
            "baseUrl": base_url,
            "defaultModel": "qwen3-coder-plus"
        }),
        storage_mode: "inline".to_string(),
        cooldown_until: None,
        last_error: None,
        failure_count: 0,
        last_health_check_at: None,
        created_at: "2026-04-20T00:00:00Z".to_string(),
        updated_at: "2026-04-20T00:00:00Z".to_string(),
    }
}

pub(super) fn build_test_credential(
    id: &str,
    sync_mode: &str,
    source_path: Option<&str>,
    status: &str,
    archived_at: Option<&str>,
) -> db::GatewayProviderCredentialView {
    db::GatewayProviderCredentialView {
        id: id.to_string(),
        provider_account_id: "provider-1".to_string(),
        label: format!("Credential {id}"),
        status: status.to_string(),
        payload: serde_json::json!({}),
        storage_mode: "inline".to_string(),
        source_kind: "folder_sync_import".to_string(),
        source_path: source_path.map(|value| value.to_string()),
        source_hash: None,
        sync_mode: sync_mode.to_string(),
        sync_state: "idle".to_string(),
        sync_error: None,
        cooldown_until: None,
        last_error: None,
        failure_count: 0,
        last_health_check_at: None,
        created_at: "2026-04-18T00:00:00Z".to_string(),
        updated_at: "2026-04-18T00:00:00Z".to_string(),
        archived_at: archived_at.map(|value| value.to_string()),
    }
}
