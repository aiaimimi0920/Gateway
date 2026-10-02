//! Accio credential-aware catalog requests and control-plane headers.

use super::build_absolute_url;
use super::model_catalog::{
    extract_accio_catalog_model_names, filter_model_names_by_payload_constraints,
    normalize_model_names, read_configured_supported_models,
};
use crate::db;
use crate::error::GatewayError;
use crate::implementation_lines;
use crate::routing::candidate::ProviderAccountPayload;
use crate::state::AppState;
use rquest::header::{HeaderMap, HeaderName, HeaderValue, CONTENT_TYPE};
use serde_json::Value;
use std::time::Duration;

pub(super) async fn fetch_accio_models_for_provider(
    state: &AppState,
    provider_account: &db::GatewayProviderAccountView,
) -> Result<Vec<String>, GatewayError> {
    implementation_lines::assert_accio_web_reverse_api_compiled(
        "Accio model catalog probe requested",
    )?;
    let mut merged_payloads = Vec::new();
    if let Some(pg_pool) = state.pg_pool.as_ref() {
        let provider_credentials = db::list_active_provider_credentials_for_accounts(
            pg_pool,
            &[provider_account.id.clone()],
        )
        .await?;
        for credential in provider_credentials {
            merged_payloads.push(db::merge_provider_account_and_credential_payloads(
                &provider_account.payload,
                &credential.payload,
            ));
        }
    }
    if merged_payloads.is_empty() {
        merged_payloads.push(provider_account.payload.clone());
    }

    let mut models = Vec::new();
    for merged_payload in merged_payloads {
        let Ok(payload) = serde_json::from_value::<ProviderAccountPayload>(merged_payload.clone())
        else {
            continue;
        };
        let fetched = fetch_accio_models_from_upstream(state, &payload, &merged_payload)
            .await
            .unwrap_or_default();
        if fetched.is_empty() {
            let configured = filter_model_names_by_payload_constraints(
                &merged_payload,
                read_configured_supported_models(&merged_payload),
            );
            if !configured.is_empty() {
                models.extend(configured);
            }
            continue;
        }
        models.extend(filter_model_names_by_payload_constraints(
            &merged_payload,
            fetched,
        ));
    }

    Ok(normalize_model_names(models))
}

async fn fetch_accio_models_from_upstream(
    state: &AppState,
    payload: &ProviderAccountPayload,
    raw_payload: &Value,
) -> Result<Vec<String>, GatewayError> {
    let base_url = payload.base_url.trim_end_matches('/');
    if base_url.is_empty() {
        return Ok(Vec::new());
    }
    let token = payload.api_key.trim().to_string();
    if token.is_empty() {
        return Ok(Vec::new());
    }
    let path = raw_payload
        .get("modelsPath")
        .or_else(|| raw_payload.get("models_path"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("/api/llm/config");

    let client = crate::http_client::builder()
        .timeout(Duration::from_secs(state.config.upstream_timeout_secs))
        .build()
        .map_err(|error| {
            GatewayError::server_error(format!("build accio catalog client: {error}"))
        })?;
    let response = client
        .post(build_absolute_url(base_url, path))
        .json(&serde_json::json!({ "token": token }))
        .headers(build_accio_control_plane_headers(payload, false))
        .send()
        .await
        .map_err(|error| {
            GatewayError::service_unavailable(format!("fetch accio catalog failed: {error}"))
        })?;
    if !response.status().is_success() {
        return Ok(Vec::new());
    }
    let body = response.json::<Value>().await.map_err(|error| {
        GatewayError::service_unavailable(format!("decode accio catalog failed: {error}"))
    })?;
    Ok(extract_accio_catalog_model_names(&body))
}

fn build_accio_control_plane_headers(
    payload: &ProviderAccountPayload,
    quota_request: bool,
) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    headers.insert(
        "accept",
        HeaderValue::from_static(if quota_request {
            "*/*"
        } else {
            "application/json"
        }),
    );
    headers.insert("user-agent", HeaderValue::from_static("node"));
    headers.insert("x-language", HeaderValue::from_static("zh"));
    headers.insert("x-os", HeaderValue::from_static("win32"));
    if quota_request {
        headers.insert("accept-language", HeaderValue::from_static("*"));
        headers.insert("sec-fetch-mode", HeaderValue::from_static("cors"));
    }
    let version = payload
        .headers
        .get("x-app-version")
        .or_else(|| payload.headers.get("version"))
        .map(String::as_str)
        .unwrap_or("0.5.6");
    insert_control_header(&mut headers, "x-app-version", version);

    if let Some(utdid) = payload
        .headers
        .get("x-utdid")
        .or_else(|| payload.headers.get("utdid"))
        .map(String::as_str)
    {
        insert_control_header(&mut headers, "x-utdid", utdid);
    }
    if let Some(cna) = payload
        .headers
        .get("x-cna")
        .map(String::as_str)
        .or_else(|| {
            payload
                .headers
                .get("Cookie")
                .or_else(|| payload.headers.get("cookie"))
                .and_then(|value| extract_cookie_value(value, "cna"))
        })
    {
        insert_control_header(&mut headers, "x-cna", cna);
    }
    headers
}

fn insert_control_header(headers: &mut HeaderMap, name: &str, value: &str) {
    let Ok(header_name) = HeaderName::from_bytes(name.as_bytes()) else {
        return;
    };
    let Ok(header_value) = HeaderValue::from_str(value) else {
        return;
    };
    headers.insert(header_name, header_value);
}

fn extract_cookie_value<'a>(cookie_header: &'a str, cookie_name: &str) -> Option<&'a str> {
    cookie_header
        .split(';')
        .filter_map(|segment| segment.split_once('='))
        .find_map(|(name, value)| {
            if name.trim().eq_ignore_ascii_case(cookie_name) {
                let trimmed = value.trim();
                (!trimmed.is_empty()).then_some(trimmed)
            } else {
                None
            }
        })
}
