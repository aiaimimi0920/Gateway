//! Normalize Codex login exports while preserving incomplete-source passthrough.

use super::chatgpt_auth::{chatgpt_web_import_string, decode_jwt_claims, json_string_at_path};
use super::source::canonicalize_folder_sync_raw_source;
use crate::db;
use crate::error::GatewayError;
use serde_json::Value;

pub(super) fn normalize_codex_import_payload(
    provider_account: &db::GatewayProviderAccountView,
    raw_payload: Value,
) -> Result<Value, GatewayError> {
    let Some(raw_map) = raw_payload.as_object() else {
        return Err(GatewayError::bad_request(
            "codex provider credential payload 必须是 JSON object",
        ));
    };

    let access_token = raw_map
        .get("access_token")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .or_else(|| {
            chatgpt_web_import_string(
                raw_map,
                &[
                    &["accessToken"],
                    &["apiKey"],
                    &["api_key"],
                    &["token"],
                    &["chatgptLoginDetails", "clientBootstrap", "accessToken"],
                ],
            )
        });
    let access_token_claims = access_token.and_then(decode_jwt_claims);
    let account_id = raw_map
        .get("account_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .or_else(|| {
            chatgpt_web_import_string(
                raw_map,
                &[
                    &["accountId"],
                    &["chatgptLoginDetails", "clientBootstrap", "accountId"],
                    &["chatgptLogin", "workspaceId"],
                    &["chatgptLogin", "personalWorkspaceId"],
                    &["workspaceId"],
                    &["personalWorkspaceId"],
                ],
            )
        })
        .or_else(|| {
            access_token_claims.as_ref().and_then(|claims| {
                json_string_at_path(
                    claims,
                    &["https://api.openai.com/auth", "chatgpt_account_id"],
                )
            })
        });

    if access_token.is_none() || account_id.is_none() {
        return Ok(raw_payload);
    }

    let base_url = provider_account
        .payload
        .get("base_url")
        .and_then(Value::as_str)
        .or_else(|| {
            provider_account
                .payload
                .get("baseUrl")
                .and_then(Value::as_str)
        })
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("https://chatgpt.com/backend-api/codex");
    let default_model = provider_account
        .payload
        .get("default_model")
        .cloned()
        .or_else(|| provider_account.payload.get("defaultModel").cloned())
        .unwrap_or_else(|| Value::String("gpt-5.4".to_string()));
    let responses_path = provider_account
        .payload
        .get("responses_path")
        .cloned()
        .or_else(|| provider_account.payload.get("responsesPath").cloned())
        .unwrap_or_else(|| Value::String("/responses".to_string()));

    Ok(serde_json::json!({
        "adapter": "openai_compatible",
        "apiKey": access_token,
        "baseUrl": base_url,
        "defaultModel": default_model,
        "responsesPath": responses_path,
        "headers": {
            "Chatgpt-Account-Id": account_id,
            "Originator": "codex_cli_rs",
            "User-Agent": "codex_cli_rs/0.1.2504151532"
        },
        "extraBody": {
            "store": false
        },
        "rawSource": canonicalize_folder_sync_raw_source(&Value::Object(raw_map.clone()))
    }))
}
