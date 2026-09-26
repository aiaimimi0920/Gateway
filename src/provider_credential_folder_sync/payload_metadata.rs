//! Material classification and metadata shared by import and export payloads.

use super::canonicalization::canonicalize_credential_material_kind;
use super::classification::derive_provider_surface_slug;
use crate::db;
use crate::protocol::gemini::shared::{
    GEMINI_API_MODULAR_PROFILE, GEMINI_CANVAS_PROGRAM_WEB_REVERSE_MODULAR_PROFILE,
    GEMINI_CANVAS_WEB_REVERSE_MODULAR_PROFILE, GEMINI_WEB_REVERSE_MODULAR_PROFILE,
};
use serde_json::Value;

pub(super) fn materialize_export_payload(
    provider_account: &db::GatewayProviderAccountView,
    payload: &Value,
) -> Value {
    let mut payload_map = payload.as_object().cloned().unwrap_or_default();
    apply_folder_sync_metadata(&mut payload_map, provider_account, None);
    Value::Object(payload_map)
}

pub(super) fn derive_credential_material_kind(
    provider_account: &db::GatewayProviderAccountView,
    payload: Option<&Value>,
) -> String {
    if let Some(raw_map) = payload.and_then(Value::as_object) {
        if let Some(kind) = raw_map
            .get("credentialMaterialKind")
            .or_else(|| raw_map.get("credential_material_kind"))
            .or_else(|| raw_map.get("materialKind"))
            .or_else(|| raw_map.get("material_kind"))
            .and_then(Value::as_str)
        {
            let trimmed = kind.trim();
            if !trimmed.is_empty() {
                return canonicalize_credential_material_kind(trimmed);
            }
        }
        if raw_map.contains_key("runtimeStateObjectKey")
            || raw_map.contains_key("runtime_state_object_key")
        {
            return "browser_state".to_string();
        }
        if raw_map
            .get("extraBody")
            .or_else(|| raw_map.get("extra_body"))
            .and_then(Value::as_object)
            .is_some_and(|extra| {
                extra.contains_key("configId")
                    || extra.contains_key("config_id")
                    || extra.contains_key("session")
            })
        {
            return "jwt_widget_session".to_string();
        }
    }

    if let Some(kind) = default_credential_material_kind_for_protocol_profile(
        provider_account.protocol_profile.as_str(),
    ) {
        return kind.to_string();
    }

    match provider_account.protocol_profile.as_str() {
        "qwen_dashscope_openai" | "qwen_coding_plan_openai" | "qwen_coding_plan_anthropic" => {
            "api_key".to_string()
        }
        "chatgpt_web_reverse" => "session_auth".to_string(),
        "qwen_web_chat" | "accio" | "chataibot" | "suno" | "udio" | "lumalabs" | "freebuff"
        | "producer" => "session_auth".to_string(),
        _ => "api_key".to_string(),
    }
}

fn default_credential_material_kind_for_protocol_profile(
    protocol_profile: &str,
) -> Option<&'static str> {
    match protocol_profile {
        "azure_openai" => Some("api_key"),
        "anthropic" => Some("api_key"),
        "aws_bedrock" => Some("bearer_token"),
        "cohere" => Some("api_key"),
        "google_gemini_api" | GEMINI_API_MODULAR_PROFILE => Some("api_key"),
        "google_vertex_gemini" => Some("bearer_token"),
        "gemini_web" | GEMINI_WEB_REVERSE_MODULAR_PROFILE => Some("session_auth"),
        "gemini_business" => Some("jwt_widget_session"),
        "gemini_canvas"
        | GEMINI_CANVAS_WEB_REVERSE_MODULAR_PROFILE
        | GEMINI_CANVAS_PROGRAM_WEB_REVERSE_MODULAR_PROFILE => Some("browser_state"),
        "xai" | "xai_openai" | "perplexity_chat" | "xfyun_openai" | "xfyun_native_websocket" => {
            Some("api_key")
        }
        "freebuff" | "producer" => Some("session_auth"),
        "kiro" => Some("bearer_token"),
        _ => None,
    }
}

pub(super) fn apply_folder_sync_metadata(
    payload: &mut serde_json::Map<String, Value>,
    provider_account: &db::GatewayProviderAccountView,
    credential_material_kind_hint: Option<&str>,
) {
    payload
        .entry("serviceProviderKey".to_string())
        .or_insert_with(|| Value::String(provider_account.service_provider_key.clone()));
    payload
        .entry("providerSurfaceKey".to_string())
        .or_insert_with(|| Value::String(derive_provider_surface_slug(provider_account)));

    let material_kind = credential_material_kind_hint
        .map(canonicalize_credential_material_kind)
        .unwrap_or_else(|| {
            derive_credential_material_kind(provider_account, Some(&Value::Object(payload.clone())))
        });
    payload
        .entry("credentialMaterialKind".to_string())
        .or_insert_with(|| Value::String(material_kind));
}
