//! Ordinary workspace API identity and send-time policy, independent of the Qwen feature gates.
//! The marker preserves product identity if a later merge changes the destination or adapter.
use crate::{
    error::GatewayError,
    provider_discovery::model_catalog::qwen_endpoint::QwenEndpoint,
    routing::candidate::{ProviderAccountPayload, ProviderExecutionMode},
};

pub const AUTH_MODE: &str = "qwen_workspace_api";
pub const PROFILE: &str = "qwen_official_api";

pub fn owns_payload(payload: &ProviderAccountPayload) -> bool {
    payload.auth_mode.as_deref() == Some(AUTH_MODE)
        || QwenEndpoint::parse(&payload.base_url).is_some()
}

pub fn validate(payload: &ProviderAccountPayload, adapter: &str) -> Result<(), GatewayError> {
    if !owns_payload(payload) {
        return Ok(());
    }
    if !cfg!(feature = "line-qwen-official-api")
        || !cfg!(feature = "family-openai-compatible-official-api")
    {
        return Err(GatewayError::conflict(
            "Qwen workspace API implementation line was compiled out.",
        )
        .with_code("gateway_provider_line_compiled_out"));
    }
    if QwenEndpoint::parse(&payload.base_url).is_none()
        || payload.adapter != "openai_compatible"
        || adapter != "openai_compatible"
        || !matches!(
            payload.auth_mode.as_deref(),
            None | Some("bearer") | Some(AUTH_MODE)
        )
        || payload.session_auth.is_some()
        || payload.keepalive.is_some()
        || payload.auth_header_name.is_some()
        || payload.auth_token.is_some()
        || !payload.discovered_protocols.is_empty()
        || payload.responses_path.is_some()
        || payload.messages_path.is_some()
        || payload.completions_path.is_some()
        || payload.embeddings_path.is_some()
        || payload.audio_transcriptions_path.is_some()
        || payload.audio_speech_path.is_some()
        || payload.search_path.is_some()
        || payload.fetch_path.is_some()
        || payload.research_path.is_some()
        || payload.balance_path.is_some()
        || payload
            .chat_completions_path
            .as_deref()
            .is_some_and(|path| path != "/chat/completions")
        || payload
            .execution_mode
            .is_some_and(|mode| mode != ProviderExecutionMode::DirectHttp)
        || payload
            .endpoint_execution_modes
            .as_ref()
            .is_some_and(|modes| {
                modes
                    .values()
                    .any(|mode| *mode != ProviderExecutionMode::DirectHttp)
            })
        || payload.headers.keys().any(|name| {
            matches!(
                name.to_ascii_lowercase().as_str(),
                "authorization" | "cookie" | "x-api-key" | "api-key" | "host"
            )
        })
    {
        return Err(invalid());
    }
    Ok(())
}

pub fn validate_key(payload: &ProviderAccountPayload) -> Result<(), GatewayError> {
    if payload.api_key.is_empty()
        || payload.api_key.len() > 16 * 1024
        || !payload.api_key.bytes().all(|byte| byte.is_ascii_graphic())
    {
        return Err(invalid());
    }
    Ok(())
}

fn invalid() -> GatewayError {
    GatewayError::bad_request("Invalid ordinary Qwen workspace API configuration.")
        .with_code("invalid_qwen_workspace_api")
}
