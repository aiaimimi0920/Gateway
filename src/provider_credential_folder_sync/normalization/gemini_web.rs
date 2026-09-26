//! Normalize Gemini Web cookies and bootstrap metadata.

use super::super::payload_metadata::apply_folder_sync_metadata;
use super::source::{canonicalize_folder_sync_raw_source, read_optional_object_string};
use crate::db;
use crate::error::GatewayError;
use serde_json::Value;

pub(super) fn normalize_gemini_web_import_payload(
    provider_account: &db::GatewayProviderAccountView,
    raw_payload: Value,
    credential_material_kind_hint: Option<&str>,
) -> Result<Value, GatewayError> {
    let Some(raw_map) = raw_payload.as_object() else {
        return Err(GatewayError::bad_request(
            "gemini web provider credential payload 必须是 JSON object",
        ));
    };

    let api_key = raw_map
        .get("apiKey")
        .or_else(|| raw_map.get("api_key"))
        .or_else(|| raw_map.get("secure_1psid"))
        .or_else(|| raw_map.get("__Secure-1PSID"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            GatewayError::bad_request("gemini web provider credential payload 缺少 __Secure-1PSID")
        })?;

    let auth_token = raw_map
        .get("authToken")
        .or_else(|| raw_map.get("auth_token"))
        .or_else(|| raw_map.get("secure_1psidts"))
        .or_else(|| raw_map.get("__Secure-1PSIDTS"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);

    let cookie_header = raw_map
        .get("cookieHeader")
        .or_else(|| raw_map.get("cookie_header"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| {
            raw_map
                .get("headers")
                .and_then(Value::as_object)
                .and_then(|headers| {
                    headers
                        .get("Cookie")
                        .or_else(|| headers.get("cookie"))
                        .and_then(Value::as_str)
                })
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
        });

    let default_model = read_optional_object_string(
        raw_map,
        &[
            "defaultModel",
            "default_model",
            "model",
            "selectedModel",
            "selected_model",
        ],
    );
    let credential_material_key = read_optional_object_string(
        raw_map,
        &[
            "credentialMaterialKey",
            "credential_material_key",
            "materialKey",
            "material_key",
        ],
    )
    .or_else(|| {
        read_optional_object_string(raw_map, &["accountName", "account_name"])
            .map(|value| format!("gemini-web-session:{value}"))
    });

    let mut payload = serde_json::Map::new();
    payload.insert("apiKey".to_string(), Value::String(api_key.to_string()));
    if let Some(auth_token) = auth_token {
        payload.insert("authToken".to_string(), Value::String(auth_token));
    }
    if let Some(default_model) = default_model {
        payload.insert(
            "supportedModels".to_string(),
            Value::Array(vec![Value::String(default_model.clone())]),
        );
        payload.insert("defaultModel".to_string(), Value::String(default_model));
    }
    if let Some(cookie_header) = cookie_header {
        payload.insert(
            "headers".to_string(),
            serde_json::json!({
                "Cookie": cookie_header
            }),
        );
    }
    if let Some(credential_material_key) = credential_material_key {
        payload.insert(
            "credentialMaterialKey".to_string(),
            Value::String(credential_material_key),
        );
    }

    let bootstrap_keys = [
        ("accessToken", "accessToken"),
        ("access_token", "accessToken"),
        ("buildLabel", "buildLabel"),
        ("build_label", "buildLabel"),
        ("sessionId", "sessionId"),
        ("session_id", "sessionId"),
        ("language", "language"),
        ("pushId", "pushId"),
        ("push_id", "pushId"),
        ("modelHeader", "modelHeader"),
        ("model_header", "modelHeader"),
        ("requestContextHeader", "requestContextHeader"),
        ("request_context_header", "requestContextHeader"),
    ];
    let mut extra_body = serde_json::Map::new();
    for (raw_key, normalized_key) in bootstrap_keys {
        if let Some(value) = raw_map.get(raw_key).cloned() {
            extra_body.insert(normalized_key.to_string(), value);
        }
    }
    if let Some(model_headers) = raw_map.get("modelHeaders").cloned() {
        extra_body.insert("modelHeaders".to_string(), model_headers);
    }
    if !extra_body.is_empty() {
        payload.insert("extraBody".to_string(), Value::Object(extra_body));
    }

    apply_folder_sync_metadata(
        &mut payload,
        provider_account,
        credential_material_kind_hint.or(Some("session_auth")),
    );
    payload.insert(
        "rawSource".to_string(),
        canonicalize_folder_sync_raw_source(&raw_payload),
    );
    Ok(Value::Object(payload))
}
