//! Normalize Accio account exports, including account identity from cookies.

use super::super::payload_metadata::apply_folder_sync_metadata;
use super::source::{
    canonicalize_folder_sync_raw_source, extract_cookie_value, read_optional_object_string,
};
use crate::db;
use crate::error::GatewayError;
use crate::implementation_lines;
use serde_json::Value;

pub(super) fn normalize_accio_import_payload(
    provider_account: &db::GatewayProviderAccountView,
    raw_payload: Value,
) -> Result<Value, GatewayError> {
    implementation_lines::assert_accio_web_reverse_api_compiled(
        "compiled-out Accio import normalization requested",
    )?;
    let Some(raw_map) = raw_payload.as_object() else {
        return Err(GatewayError::bad_request(
            "accio provider credential payload 必须是 JSON object",
        ));
    };

    let nested_headers = raw_map.get("headers").and_then(Value::as_object);
    let nested_extra_body = raw_map
        .get("extraBody")
        .or_else(|| raw_map.get("extra_body"))
        .and_then(Value::as_object);

    let access_token = read_optional_object_string(
        raw_map,
        &["accessToken", "access_token", "apiKey", "api_key"],
    )
    .or_else(|| {
        nested_extra_body.and_then(|extra| {
            read_optional_object_string(extra, &["accessToken", "access_token", "token"])
        })
    });
    if access_token.is_none() {
        return Ok(raw_payload);
    }
    let access_token = access_token.unwrap_or_default();
    let utdid = read_optional_object_string(raw_map, &["utdid", "x-utdid"])
        .or_else(|| {
            nested_headers
                .and_then(|headers| read_optional_object_string(headers, &["utdid", "x-utdid"]))
        })
        .ok_or_else(|| GatewayError::bad_request("accio provider credential payload 缺少 utdid"))?;

    let base_url = provider_account
        .payload
        .get("baseUrl")
        .and_then(Value::as_str)
        .or_else(|| {
            provider_account
                .payload
                .get("base_url")
                .and_then(Value::as_str)
        })
        .or_else(|| raw_map.get("baseUrl").and_then(Value::as_str))
        .or_else(|| raw_map.get("base_url").and_then(Value::as_str))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("https://phoenix-gw.alibaba.com");
    let version = provider_account
        .payload
        .get("headers")
        .and_then(Value::as_object)
        .and_then(|headers| {
            headers
                .get("x-app-version")
                .or_else(|| headers.get("version"))
                .and_then(Value::as_str)
        })
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| {
            nested_headers.and_then(|headers| {
                read_optional_object_string(headers, &["version", "x-app-version"])
            })
        })
        .unwrap_or_else(|| "0.5.9".to_string());
    let default_model = provider_account
        .payload
        .get("defaultModel")
        .cloned()
        .or_else(|| provider_account.payload.get("default_model").cloned())
        .unwrap_or_else(|| Value::String("claude-sonnet-4-6".to_string()));
    let responses_path = provider_account
        .payload
        .get("responsesPath")
        .cloned()
        .or_else(|| provider_account.payload.get("responses_path").cloned())
        .unwrap_or_else(|| Value::String("/api/adk/llm/generateContent".to_string()));

    let refresh_token = raw_map
        .get("refreshToken")
        .or_else(|| raw_map.get("refresh_token"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| {
            nested_extra_body.and_then(|extra| {
                read_optional_object_string(extra, &["refreshToken", "refresh_token"])
            })
        });
    let cookie_header =
        read_optional_object_string(raw_map, &["cookie", "cookieHeader"]).or_else(|| {
            nested_headers
                .and_then(|headers| read_optional_object_string(headers, &["Cookie", "cookie"]))
        });
    let expires_at = raw_map
        .get("expiresAt")
        .or_else(|| raw_map.get("expires_at"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let account_name =
        read_optional_object_string(raw_map, &["accountName", "email", "name", "id"]);
    let credential_material_key = raw_map
        .get("credentialMaterialKey")
        .or_else(|| raw_map.get("materialKey"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| {
            raw_map
                .get("id")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(|account_id| format!("accio-account:{account_id}"))
        });
    let excluded_models = raw_map
        .get("disabledModels")
        .or_else(|| raw_map.get("disabled_models"))
        .and_then(Value::as_object)
        .map(|models| {
            models
                .keys()
                .cloned()
                .map(Value::String)
                .collect::<Vec<_>>()
        })
        .filter(|models| !models.is_empty());
    let language = read_optional_object_string(raw_map, &["x-language", "language"])
        .or_else(|| {
            nested_headers.and_then(|headers| {
                read_optional_object_string(headers, &["x-language", "language"])
            })
        })
        .unwrap_or_else(|| "zh-CN".to_string());
    let os = read_optional_object_string(raw_map, &["x-os", "os"])
        .or_else(|| {
            nested_headers.and_then(|headers| read_optional_object_string(headers, &["x-os", "os"]))
        })
        .unwrap_or_else(|| "win32".to_string());
    let empid =
        read_optional_object_string(raw_map, &["empid", "empId", "accountId", "account_id"])
            .or_else(|| {
                nested_extra_body.and_then(|extra| {
                    read_optional_object_string(
                        extra,
                        &["empid", "empId", "accountId", "account_id"],
                    )
                })
            })
            .or_else(|| {
                cookie_header
                    .as_deref()
                    .and_then(extract_accio_empid_from_cookie_header)
            })
            .or_else(|| {
                read_optional_object_string(raw_map, &["id"]).and_then(|value| {
                    let trimmed = value.trim().to_string();
                    (!trimmed.is_empty()).then_some(trimmed)
                })
            })
            .unwrap_or_default();
    let tenant = read_optional_object_string(raw_map, &["tenant", "tenantId", "tenant_id"])
        .or_else(|| {
            nested_extra_body.and_then(|extra| {
                read_optional_object_string(extra, &["tenant", "tenantId", "tenant_id"])
            })
        })
        .unwrap_or_default();
    let iai_tag = read_optional_object_string(raw_map, &["iaiTag", "iai_tag"])
        .or_else(|| {
            nested_extra_body
                .and_then(|extra| read_optional_object_string(extra, &["iaiTag", "iai_tag"]))
        })
        .unwrap_or_else(|| "phoenix-desktop".to_string());
    let app_key = provider_account
        .payload
        .get("headers")
        .and_then(Value::as_object)
        .and_then(|headers| {
            headers
                .get("appKey")
                .or_else(|| headers.get("app_key"))
                .and_then(Value::as_str)
        })
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| {
            nested_headers
                .and_then(|headers| read_optional_object_string(headers, &["appKey", "app_key"]))
        })
        .or_else(|| Some("35306229".to_string()));

    let mut headers = serde_json::Map::new();
    headers.insert("utdid".to_string(), Value::String(utdid.to_string()));
    headers.insert("x-utdid".to_string(), Value::String(utdid.to_string()));
    headers.insert("version".to_string(), Value::String(version.to_string()));
    headers.insert(
        "x-app-version".to_string(),
        Value::String(version.to_string()),
    );
    headers.insert("x-language".to_string(), Value::String(language));
    headers.insert("x-os".to_string(), Value::String(os));
    if let Some(cookie_header) = cookie_header.clone() {
        headers.insert("Cookie".to_string(), Value::String(cookie_header.clone()));
        if let Some(cna) = extract_cookie_value(&cookie_header, "cna") {
            headers.insert("x-cna".to_string(), Value::String(cna.to_string()));
        }
    }
    if let Some(app_key) = app_key {
        headers.insert("appKey".to_string(), Value::String(app_key));
    }

    let mut extra_body = serde_json::Map::new();
    extra_body.insert("token".to_string(), Value::String(access_token.to_string()));
    extra_body.insert(
        "accessToken".to_string(),
        Value::String(access_token.to_string()),
    );
    extra_body.insert("empid".to_string(), Value::String(empid));
    extra_body.insert("tenant".to_string(), Value::String(tenant));
    extra_body.insert("iaiTag".to_string(), Value::String(iai_tag));
    extra_body.insert("utdid".to_string(), Value::String(utdid.to_string()));
    extra_body.insert("version".to_string(), Value::String(version.to_string()));

    let mut payload = serde_json::Map::new();
    payload.insert(
        "apiKey".to_string(),
        Value::String(access_token.to_string()),
    );
    payload.insert("baseUrl".to_string(), Value::String(base_url.to_string()));
    payload.insert("defaultModel".to_string(), default_model);
    payload.insert("responsesPath".to_string(), responses_path);
    payload.insert("headers".to_string(), Value::Object(headers));
    payload.insert("extraBody".to_string(), Value::Object(extra_body));
    if let Some(refresh_token) = refresh_token {
        payload.insert("refreshToken".to_string(), Value::String(refresh_token));
    }
    if let Some(expires_at) = expires_at {
        payload.insert("expiresAt".to_string(), Value::String(expires_at));
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
    if let Some(excluded_models) = excluded_models {
        payload.insert("excludedModels".to_string(), Value::Array(excluded_models));
    }
    apply_folder_sync_metadata(&mut payload, provider_account, Some("session_auth"));
    payload.insert(
        "rawSource".to_string(),
        canonicalize_folder_sync_raw_source(&raw_payload),
    );
    Ok(Value::Object(payload))
}

fn extract_accio_empid_from_cookie_header(cookie_header: &str) -> Option<String> {
    let xman_us_f = extract_cookie_value(cookie_header, "xman_us_f")?;
    for segment in xman_us_f.split('&') {
        let (name, value) = segment.split_once('=')?;
        if !name.trim().eq_ignore_ascii_case("x_user") {
            continue;
        }
        let empid = value
            .split('|')
            .next_back()
            .map(str::trim)
            .filter(|entry| !entry.is_empty())?;
        return Some(empid.to_string());
    }
    None
}
