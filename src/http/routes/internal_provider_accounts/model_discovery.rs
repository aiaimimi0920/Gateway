//! Provider model discovery fallback order and generic catalog transport.

use super::accio_catalog::fetch_accio_models_for_provider;
use super::build_absolute_url;
use super::model_catalog::{
    extract_model_names_from_value, normalize_model_names, provider_model_key,
    read_configured_supported_models, read_default_model_from_payload,
};
use super::tiering_storage::ProviderCapabilityTieringRow;
use crate::db;
use crate::error::GatewayError;
use crate::routing::candidate::ProviderAccountPayload;
use crate::state::AppState;
use crate::upstream::headers::build_upstream_headers;
use serde_json::Value;
use std::time::Duration;

pub(crate) fn make_provider_payload_serde_compatible(
    provider_account: &db::GatewayProviderAccountView,
    payload: &Value,
) -> Value {
    let mut payload = payload.clone();
    if let Some(object) = payload.as_object_mut() {
        if !object.contains_key("adapter") {
            object.insert(
                "adapter".to_string(),
                Value::String(provider_account.adapter.clone()),
            );
        }
        if !object.contains_key("baseUrl") && !object.contains_key("base_url") {
            object.insert("baseUrl".to_string(), Value::String(String::new()));
        }
        if !object.contains_key("apiKey") && !object.contains_key("api_key") {
            object.insert("apiKey".to_string(), Value::String(String::new()));
        }
    }
    payload
}

pub(super) fn deserialize_provider_payload_for_model_tiering(
    provider_account: &db::GatewayProviderAccountView,
) -> Result<ProviderAccountPayload, GatewayError> {
    let payload =
        make_provider_payload_serde_compatible(provider_account, &provider_account.payload);
    serde_json::from_value(payload).map_err(|error| {
        GatewayError::server_error(format!(
            "deserialize provider payload for model tiering {}: {error}",
            provider_account.id
        ))
    })
}

pub(super) async fn discover_provider_models(
    state: &AppState,
    provider_account: &db::GatewayProviderAccountView,
    payload: &ProviderAccountPayload,
    capabilities: &[ProviderCapabilityTieringRow],
) -> (Vec<String>, String) {
    if payload.canonical_adapter() == "accio_compatible" {
        let accio_models = fetch_accio_models_for_provider(state, provider_account)
            .await
            .unwrap_or_default();
        if !accio_models.is_empty() {
            return (accio_models, "accio_catalog_live".to_string());
        }
    }

    let upstream_models = fetch_provider_models_from_upstream(state, provider_account, payload)
        .await
        .unwrap_or_default();
    if !upstream_models.is_empty() {
        return (upstream_models, "upstream_models".to_string());
    }

    let configured_models = read_configured_supported_models(&provider_account.payload);
    if !configured_models.is_empty() {
        return (configured_models, "configured_supported_models".to_string());
    }

    let capability_models = normalize_model_names(
        capabilities
            .iter()
            .map(|row| provider_model_key(&row.model_code, row.upstream_model.as_deref()))
            .collect(),
    );
    if !capability_models.is_empty() {
        return (capability_models, "capability_catalog".to_string());
    }

    let default_model = read_default_model_from_payload(&provider_account.payload, payload)
        .map(|model| vec![model])
        .unwrap_or_default();
    if !default_model.is_empty() {
        return (default_model, "default_model".to_string());
    }

    (Vec::new(), "capability_catalog".to_string())
}

async fn fetch_provider_models_from_upstream(
    state: &AppState,
    provider_account: &db::GatewayProviderAccountView,
    payload: &ProviderAccountPayload,
) -> Result<Vec<String>, GatewayError> {
    if crate::protocol::kiro_oauth::runtime::owns_payload(payload) {
        return Err(crate::protocol::kiro_oauth::runtime::invalid());
    }
    if crate::protocol::kiro_api_key::owns_payload(payload) {
        return Err(crate::protocol::kiro_api_key::invalid());
    }
    // No generic catalog URL may consume a Build token, including an absolute modelsPath override.
    if crate::protocol::grok_oauth::business::owns_payload(payload) {
        return Err(crate::protocol::grok_oauth::business::invalid());
    }
    if crate::protocol::chatgpt::official_api::is_chatgpt_codex_backend_payload(payload) {
        let account = payload
            .headers
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case("Chatgpt-Account-Id"))
            .map(|(_, value)| value.as_str())
            .unwrap_or_default();
        return crate::protocol::chatgpt::codex_client::models(
            state.upstream_client.client(),
            &payload.api_key,
            account,
        )
        .await;
    }
    let Some(url) = generic_models_url(payload, &provider_account.payload)? else {
        return Ok(Vec::new());
    };

    let client = crate::http_client::builder()
        .timeout(Duration::from_secs(state.config.upstream_timeout_secs))
        .build()
        .map_err(|error| {
            GatewayError::server_error(format!("build provider models client: {error}"))
        })?;
    let response = client
        .get(url)
        .headers(build_upstream_headers(payload))
        .send()
        .await
        .map_err(|error| {
            GatewayError::service_unavailable(format!("fetch provider models failed: {error}"))
        })?;
    if !response.status().is_success() {
        return Ok(Vec::new());
    }
    let body = response.json::<Value>().await.map_err(|error| {
        GatewayError::service_unavailable(format!("decode provider models failed: {error}"))
    })?;
    Ok(extract_model_names_from_value(&body))
}

fn generic_models_url(
    payload: &ProviderAccountPayload,
    raw: &Value,
) -> Result<Option<String>, GatewayError> {
    crate::protocol::lobsterai::management_boundary::reject_generic(payload)?;
    crate::protocol::luma_mcp::boundary::reject_generic(payload)?;
    if crate::protocol::commandcode::owns_payload(payload) {
        return Err(crate::protocol::commandcode::invalid());
    }
    if crate::protocol::sap_aicore::owns_payload(payload) {
        return Err(crate::protocol::sap_aicore::invalid());
    }
    if crate::protocol::kiro_oauth::runtime::owns_payload(payload) {
        return Err(crate::protocol::kiro_oauth::runtime::invalid());
    }
    if crate::protocol::kiro_api_key::owns_payload(payload) {
        return Err(crate::protocol::kiro_api_key::invalid());
    }
    if crate::protocol::grok_oauth::business::owns_payload(payload) {
        return Err(crate::protocol::grok_oauth::business::invalid());
    }
    let base_url = payload.base_url.trim_end_matches('/');
    if base_url.is_empty() {
        return Ok(None);
    }
    let configured_path = raw
        .get("modelsPath")
        .or_else(|| raw.get("models_path"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    let path = configured_path.or_else(|| match payload.canonical_adapter() {
        "openai_compatible" | "anthropic_compatible" => Some("/models".to_string()),
        "chatgpt_web_reverse_compatible" => {
            Some(crate::protocol::chatgpt::web_reverse::CHATGPT_WEB_DEFAULT_MODELS_PATH.to_string())
        }
        _ => None,
    });
    Ok(path.map(|path| build_absolute_url(base_url, &path)))
}

#[cfg(test)]
#[test]
fn luma_mcp_generic_model_path_override_is_rejected() {
    let payload = serde_json::from_value(serde_json::json!({
        "adapter":"openai_compatible","auth_mode":"luma_mcp_oauth",
        "base_url":"https://other.invalid","api_key":"synthetic"}))
    .unwrap();
    let error = generic_models_url(
        &payload,
        &serde_json::json!({
        "modelsPath":"http://127.0.0.1:1/models"}),
    )
    .unwrap_err();
    assert_eq!(error.code.as_deref(), Some("luma_mcp_management_only"));
}

#[cfg(test)]
mod grok_oauth_tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn commandcode_generic_catalog_rejects_default_and_absolute_urls() {
        let payload: ProviderAccountPayload = serde_json::from_value(json!({
            "adapter":"openai_compatible","auth_mode":crate::protocol::commandcode::AUTH_MODE,
            "base_url":crate::protocol::commandcode::BASE,"api_key":"synthetic-commandcode"}))
        .unwrap();
        for raw in [
            json!({}),
            json!({"models_url":"https://foreign.invalid/models"}),
        ] {
            let error = generic_models_url(&payload, &raw).unwrap_err();
            assert_eq!(error.code.as_deref(), Some("invalid_commandcode_chat"));
        }
    }
    #[test]
    fn sap_aicore_generic_catalog_rejects_default_and_absolute_urls() {
        let payload: ProviderAccountPayload = serde_json::from_value(json!({
            "adapter":"openai_compatible","auth_mode":crate::protocol::sap_aicore::AUTH_MODE,
            "base_url":crate::protocol::sap_aicore::test_support::BASE,
            "api_key":crate::protocol::sap_aicore::test_support::KEY
        }))
        .unwrap();
        for raw in [
            json!({}),
            json!({"modelsPath":"https://foreign.invalid/models"}),
            json!({"models_path":"/models"}),
        ] {
            assert!(generic_models_url(&payload, &raw).is_err());
        }
    }
    #[test]
    fn grok_oauth_generic_catalog_never_builds_an_authenticated_url() {
        for marker in [true, false] {
            let payload: ProviderAccountPayload = serde_json::from_value(json!({
                "adapter":"openai_compatible","base_url":crate::protocol::grok_oauth::BUSINESS_BASE_URL,
                "api_key":"synthetic-build-only","auth_mode":if marker {Some(crate::protocol::grok_oauth::business::AUTH_MODE)} else {None},
            })).unwrap();
            for raw in [
                json!({}),
                json!({"modelsPath":"https://foreign.invalid/models"}),
                json!({"models_path":"/models"}),
            ] {
                assert!(generic_models_url(&payload, &raw).is_err());
            }
        }
    }

    #[test]
    fn kiro_oauth_runtime_generic_catalog_never_builds_an_authenticated_url() {
        let payload: ProviderAccountPayload = serde_json::from_value(json!({
            "adapter":"kiro_compatible","auth_mode":"kiro_oauth_runtime",
            "base_url":"https://runtime.eu-central-1.kiro.dev","api_key":"synthetic-oauth-access"
        }))
        .unwrap();
        assert!(!crate::protocol::kiro_api_key::owns_payload(&payload));
        for raw in [
            json!({}),
            json!({"modelsPath":"https://foreign.invalid/models"}),
            json!({"models_path":"/models"}),
        ] {
            assert!(generic_models_url(&payload, &raw).is_err());
        }
    }
}
