use crate::error::GatewayError;
use crate::protocol::chatgpt::web_reverse as surface;
use crate::routing::candidate::ProviderAccountPayload;
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct ChatGptWebRequestContext {
    pub site_base_url: String,
    pub origin: String,
    pub referer: String,
    pub user_agent: String,
    pub accept_language: String,
    pub language_code: String,
    pub client_version: String,
    pub client_build_number: String,
    pub device_id: String,
    pub session_id: String,
}

pub fn unsupported_request_plan_error() -> GatewayError {
    GatewayError::bad_request(
        "ChatGPT Web reverse adapters use site bootstrap + sentinel + conversation replay and are not supported by the generic request planner.",
    )
    .with_code("unsupported_chatgpt_web_reverse_request_plan")
}

pub fn build_request_context(payload: &ProviderAccountPayload) -> ChatGptWebRequestContext {
    let site_base_url = surface::normalize_site_base_url(&payload.base_url);
    let origin = site_base_url.clone();
    let referer = format!("{origin}/");

    ChatGptWebRequestContext {
        site_base_url,
        origin,
        referer,
        user_agent: extra_string(payload, &["userAgent"])
            .unwrap_or_else(|| surface::CHATGPT_WEB_DEFAULT_USER_AGENT.to_string()),
        accept_language: extra_string(payload, &["language"])
            .unwrap_or_else(|| surface::CHATGPT_WEB_DEFAULT_ACCEPT_LANGUAGE.to_string()),
        language_code: extra_string(payload, &["languageCode", "oaiLanguage"])
            .unwrap_or_else(|| surface::CHATGPT_WEB_DEFAULT_LANGUAGE.to_string()),
        client_version: extra_string(payload, &["clientVersion"])
            .unwrap_or_else(|| surface::CHATGPT_WEB_DEFAULT_CLIENT_VERSION.to_string()),
        client_build_number: extra_string(payload, &["clientBuildNumber"])
            .unwrap_or_else(|| surface::CHATGPT_WEB_DEFAULT_CLIENT_BUILD_NUMBER.to_string()),
        device_id: extra_string(payload, &["deviceId", "oaiDeviceId"])
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
        session_id: extra_string(payload, &["sessionId", "oaiSessionId"])
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
    }
}

pub fn build_target_url(context: &ChatGptWebRequestContext, path: &str) -> String {
    if path.starts_with("http://") || path.starts_with("https://") {
        return path.to_string();
    }
    if path.starts_with('/') {
        format!("{}{}", context.site_base_url, path)
    } else {
        format!("{}/{}", context.site_base_url, path)
    }
}

fn extra_string(payload: &ProviderAccountPayload, keys: &[&str]) -> Option<String> {
    if let Some(value) = payload
        .extra_body
        .as_ref()
        .and_then(|body| keys.iter().find_map(|key| body.get(*key)))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return Some(value.to_string());
    }
    payload
        .headers
        .iter()
        .find_map(|(key, value)| {
            let normalized_key = normalize_lookup_key(key);
            keys.iter()
                .any(|candidate| normalize_lookup_key(candidate) == normalized_key)
                .then_some(value.trim())
        })
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn normalize_lookup_key(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(|character| character.to_lowercase())
        .collect()
}

pub use surface::{
    build_legacy_requirements_token, build_proof_token, merge_bootstrap_from_fallback,
    solve_turnstile_token, translate_chatgpt_web_stream,
};
