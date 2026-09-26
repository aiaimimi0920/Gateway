//! Normalize ChataiBot token and browser-session imports.

use super::super::payload_metadata::apply_folder_sync_metadata;
use super::source::{
    canonicalize_folder_sync_raw_source, extract_cookie_value, read_optional_object_string,
    read_optional_object_string_array,
};
use crate::db;
use crate::error::GatewayError;
use serde_json::Value;

pub(super) fn normalize_chataibot_import_payload(
    provider_account: &db::GatewayProviderAccountView,
    raw_payload: Value,
) -> Result<Value, GatewayError> {
    let Some(raw_map) = raw_payload.as_object() else {
        return Err(GatewayError::bad_request(
            "chataibot image provider credential payload 必须是 JSON object",
        ));
    };

    let cookie_header = read_optional_object_string(raw_map, &["cookieHeader", "cookie_header"])
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

    let auth_token = read_optional_object_string(
        raw_map,
        &["authToken", "auth_token", "apiKey", "api_key", "token"],
    )
    .or_else(|| {
        cookie_header
            .as_deref()
            .and_then(|cookie| extract_cookie_value(cookie, "token"))
            .map(str::to_string)
    })
    .ok_or_else(|| {
        GatewayError::bad_request("chataibot image provider credential payload 缺少 auth token")
    })?;

    let normalized_cookie_header = match cookie_header {
        Some(cookie) => {
            let trimmed = cookie.trim();
            if trimmed.is_empty() {
                Some(format!("token={auth_token}"))
            } else if trimmed.to_ascii_lowercase().contains("token=") {
                Some(trimmed.to_string())
            } else {
                Some(format!("token={auth_token}; {trimmed}"))
            }
        }
        None => Some(format!("token={auth_token}")),
    };

    let expires_at = read_optional_object_string(raw_map, &["expiresAt", "expires_at"]);
    let supported_models =
        read_optional_object_string_array(raw_map, &["supportedModels", "supported_models"])
            .or_else(|| {
                read_optional_object_string(
                    raw_map,
                    &[
                        "selectedModel",
                        "selected_model",
                        "model",
                        "defaultModel",
                        "default_model",
                    ],
                )
                .map(|model| vec![model])
            })
            .or_else(|| {
                provider_account
                    .payload
                    .get("defaultModel")
                    .or_else(|| provider_account.payload.get("default_model"))
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(|model| vec![model.to_string()])
            });
    let selected_display_model = read_optional_object_string(
        raw_map,
        &["selectedDisplayModel", "displayModel", "display_model"],
    );
    let account_name = read_optional_object_string(raw_map, &["accountName", "account_name"])
        .or_else(|| read_optional_object_string(raw_map, &["email", "userId", "user_id"]));
    let credential_material_key =
        read_optional_object_string(raw_map, &["credentialMaterialKey", "materialKey"]).or_else(
            || {
                read_optional_object_string(raw_map, &["userId", "user_id"])
                    .map(|user_id| format!("chataibot-user:{user_id}"))
            },
        );

    let mut payload = serde_json::Map::new();
    payload.insert("apiKey".to_string(), Value::String(auth_token));
    if let Some(cookie_header) = normalized_cookie_header {
        payload.insert(
            "headers".to_string(),
            serde_json::json!({
                "Cookie": cookie_header
            }),
        );
    }
    if let Some(expires_at) = expires_at {
        payload.insert("expiresAt".to_string(), Value::String(expires_at));
    }
    if let Some(supported_models) = supported_models {
        payload.insert(
            "supportedModels".to_string(),
            Value::Array(
                supported_models
                    .into_iter()
                    .map(Value::String)
                    .collect::<Vec<_>>(),
            ),
        );
    }
    if let Some(selected_display_model) = selected_display_model {
        payload.insert(
            "selectedDisplayModel".to_string(),
            Value::String(selected_display_model),
        );
    }
    if let Some(account_name) = account_name {
        payload.insert("accountName".to_string(), Value::String(account_name));
    }
    if let Some(credential_material_key) = credential_material_key {
        payload.insert(
            "credentialMaterialKey".to_string(),
            Value::String(credential_material_key),
        );
    }
    apply_folder_sync_metadata(&mut payload, provider_account, Some("session_auth"));
    payload.insert(
        "providerSurfaceKey".to_string(),
        Value::String("chataibot-images".to_string()),
    );
    payload.insert(
        "rawSource".to_string(),
        canonicalize_folder_sync_raw_source(&raw_payload),
    );
    Ok(Value::Object(payload))
}
