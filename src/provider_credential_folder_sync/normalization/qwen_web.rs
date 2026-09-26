//! Normalize Qwen browser-worker sessions and retained authentication seeds.

use super::super::payload_metadata::apply_folder_sync_metadata;
use super::source::canonicalize_folder_sync_raw_source;
use crate::db;
use crate::error::GatewayError;
use serde_json::Value;

pub(super) fn normalize_qwen_web_import_payload(
    provider_account: &db::GatewayProviderAccountView,
    raw_payload: Value,
) -> Result<Value, GatewayError> {
    let Some(raw_map) = raw_payload.as_object() else {
        return Err(GatewayError::bad_request(
            "qwen web provider credential payload 必须是 JSON object",
        ));
    };

    let auth_token = raw_map
        .get("apiKey")
        .or_else(|| raw_map.get("api_key"))
        .or_else(|| raw_map.get("authToken"))
        .or_else(|| raw_map.get("auth_token"))
        .or_else(|| raw_map.get("active_token"))
        .or_else(|| raw_map.get("token"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            GatewayError::bad_request("qwen web provider credential payload 缺少 auth token")
        })?;

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

    let expires_at = raw_map
        .get("expiresAt")
        .or_else(|| raw_map.get("expires_at"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);

    let selected_model = raw_map
        .get("selectedModel")
        .or_else(|| raw_map.get("selected_model"))
        .or_else(|| raw_map.get("model"))
        .or_else(|| raw_map.get("defaultModel"))
        .or_else(|| raw_map.get("default_model"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| {
            raw_map
                .get("supportedModels")
                .and_then(Value::as_array)
                .and_then(|models| models.first())
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
        })
        .or_else(|| {
            provider_account
                .payload
                .get("defaultModel")
                .or_else(|| provider_account.payload.get("default_model"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
        });

    let selected_display_model = raw_map
        .get("selectedDisplayModel")
        .or_else(|| raw_map.get("displayModel"))
        .or_else(|| raw_map.get("display_model"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);

    let account_name = raw_map
        .get("accountName")
        .or_else(|| raw_map.get("account_name"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| {
            raw_map
                .get("authProbe")
                .and_then(Value::as_object)
                .and_then(|probe| {
                    probe
                        .get("email")
                        .or_else(|| probe.get("userId"))
                        .and_then(Value::as_str)
                })
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
        });

    let credential_material_key = raw_map
        .get("credentialMaterialKey")
        .or_else(|| raw_map.get("materialKey"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| {
            raw_map
                .get("authProbe")
                .and_then(Value::as_object)
                .and_then(|probe| probe.get("userId").and_then(Value::as_str))
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(|user_id| format!("qwen-web-user:{user_id}"))
        });

    let auth_seed_value = raw_map
        .get("authSeed")
        .or_else(|| raw_map.get("auth_seed"))
        .cloned()
        .or_else(|| {
            let email = raw_map
                .get("email")
                .or_else(|| raw_map.get("loginEmail"))
                .or_else(|| raw_map.get("login_email"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string);
            let password = raw_map
                .get("password")
                .or_else(|| raw_map.get("loginPassword"))
                .or_else(|| raw_map.get("login_password"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string);
            let password_sha256 = raw_map
                .get("passwordSha256")
                .or_else(|| raw_map.get("password_sha256"))
                .or_else(|| raw_map.get("passwordHash"))
                .or_else(|| raw_map.get("password_hash"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string);

            if email.is_none() && password.is_none() && password_sha256.is_none() {
                return None;
            }

            let mut auth_seed = serde_json::Map::new();
            auth_seed.insert(
                "type".to_string(),
                Value::String("qwen_web_signin".to_string()),
            );
            if let Some(email) = email {
                auth_seed.insert("email".to_string(), Value::String(email));
            }
            if let Some(password) = password {
                auth_seed.insert("password".to_string(), Value::String(password));
            }
            if let Some(password_sha256) = password_sha256 {
                auth_seed.insert("passwordSha256".to_string(), Value::String(password_sha256));
            }
            Some(Value::Object(auth_seed))
        });

    let mut payload = serde_json::Map::new();
    payload.insert("apiKey".to_string(), Value::String(auth_token.to_string()));
    if let Some(expires_at) = expires_at {
        payload.insert("expiresAt".to_string(), Value::String(expires_at));
    }
    if let Some(cookie_header) = cookie_header {
        payload.insert(
            "headers".to_string(),
            serde_json::json!({
                "Cookie": cookie_header
            }),
        );
    }
    if let Some(selected_model) = selected_model {
        payload.insert(
            "supportedModels".to_string(),
            Value::Array(vec![Value::String(selected_model)]),
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
    if let Some(auth_seed) = auth_seed_value {
        let mut extra_body = serde_json::Map::new();
        extra_body.insert("authSeed".to_string(), auth_seed);
        payload.insert("extraBody".to_string(), Value::Object(extra_body));
    }
    apply_folder_sync_metadata(&mut payload, provider_account, Some("session_auth"));
    payload.insert(
        "rawSource".to_string(),
        canonicalize_folder_sync_raw_source(&raw_payload),
    );
    Ok(Value::Object(payload))
}
