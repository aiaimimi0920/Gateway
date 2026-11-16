//! Native CLI Key identity is static and cannot be reinterpreted as IDE/OIDC or generic API material.
use crate::{
    error::GatewayError,
    routing::candidate::{ProviderAccountPayload, ProviderExecutionMode},
};

pub const PRESET_ID: &str = "kiro-api-key";
pub const AUTH_MODE: &str = "kiro_api_key";
pub const BASE_URL: &str = "https://runtime.us-east-1.kiro.dev";
pub const TARGET: &str = "AmazonCodeWhispererStreamingService.GenerateAssistantResponse";

pub fn owns_base(base: &str) -> bool {
    url::Url::parse(base)
        .ok()
        .and_then(|u| u.host_str().map(str::to_owned))
        .is_some_and(|host| host.starts_with("runtime.") && host.ends_with(".kiro.dev"))
}
pub fn owns_payload(payload: &ProviderAccountPayload) -> bool {
    if crate::protocol::kiro_oauth::runtime::owns_payload(payload) {
        return false;
    }
    payload.auth_mode.as_deref() == Some(AUTH_MODE) || owns_base(&payload.base_url)
}
pub fn valid_material(value: &str) -> bool {
    !value.is_empty() && value.len() <= 32_768 && value.bytes().all(|b| b.is_ascii_graphic())
}
pub fn invalid() -> GatewayError {
    GatewayError::bad_request("Invalid Kiro CLI API Key product configuration.")
        .with_code("invalid_kiro_api_key")
}
pub fn validate(payload: &ProviderAccountPayload, adapter: &str) -> Result<(), GatewayError> {
    if !owns_payload(payload) {
        return Ok(());
    }
    if !cfg!(feature = "line-kiro-official-vendor-api") {
        return Err(
            GatewayError::conflict("Kiro CLI API Key transport was compiled out.")
                .with_code("gateway_provider_line_compiled_out"),
        );
    }
    if !matches!(
        payload.base_url.as_str(),
        BASE_URL | "https://runtime.us-east-1.kiro.dev/"
    ) || adapter != "kiro_compatible"
        || payload.adapter != "kiro_compatible"
        || payload.auth_mode.as_deref() != Some(AUTH_MODE)
        || payload.responses_path.as_deref() != Some("/")
        || payload.session_auth.is_some()
        || payload.keepalive.is_some()
        || payload.auth_token.is_some()
        || payload.auth_header_name.is_some()
        || !payload.headers.is_empty()
        || !payload.discovered_protocols.is_empty()
        || payload
            .extra_body
            .as_ref()
            .is_some_and(|body| !body.is_empty())
        || !matches!(
            payload.request_client_profile,
            None | Some(crate::protocol::request_profile::RequestClientProfile::Auto)
        )
        || payload
            .execution_mode
            .is_some_and(|m| m != ProviderExecutionMode::DirectHttp)
        || payload
            .endpoint_execution_modes
            .as_ref()
            .is_some_and(|m| m.values().any(|v| *v != ProviderExecutionMode::DirectHttp))
        || [
            &payload.chat_completions_path,
            &payload.messages_path,
            &payload.completions_path,
            &payload.embeddings_path,
            &payload.audio_transcriptions_path,
            &payload.audio_speech_path,
            &payload.search_path,
            &payload.fetch_path,
            &payload.research_path,
            &payload.balance_path,
        ]
        .iter()
        .any(|p| p.is_some())
        || !payload.api_key.is_empty() && !valid_material(&payload.api_key)
    {
        return Err(invalid());
    }
    Ok(())
}

/// The final transport guard also checks late planner mutations, not just the saved configuration.
pub fn validate_plan(
    payload: &ProviderAccountPayload,
    plan: &crate::upstream::common::RequestPlan,
) -> Result<(), GatewayError> {
    if !owns_payload(payload) {
        return Ok(());
    }
    validate(payload, payload.canonical_adapter())?;
    let body = plan
        .body
        .as_ref()
        .and_then(serde_json::Value::as_object)
        .ok_or_else(invalid)?;
    if !valid_material(&payload.api_key)
        || plan.method != rquest::Method::POST
        || plan.url != format!("{BASE_URL}/")
        || !plan.query.is_empty()
        || !crate::upstream::kiro::supports_endpoint(plan.response_kind)
        || body.len() != 1
        || body
            .get("conversationState")
            .is_none_or(|state| !crate::protocol::kiro::valid_runtime_state(state, payload))
    {
        return Err(invalid());
    }
    Ok(())
}

#[cfg(test)]
mod tests;
