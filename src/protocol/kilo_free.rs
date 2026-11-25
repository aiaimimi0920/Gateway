//! Public Kilo free chat. This preset never carries a Kilo account or paid route.
use crate::{
    error::{retry_after_from_headers, FallbackHint, GatewayError},
    routing::{candidate::ProviderAccountPayload, config::ProviderConfigYaml},
    upstream::common::RequestPlan,
};
use rquest::header::{HeaderMap, HeaderValue, ACCEPT, CONTENT_TYPE};

pub const PRESET: &str = "kilo-free";
pub const AUTH_MODE: &str = "kilo_anonymous_free";
pub const MODEL: &str = "kilo-auto/free";
const BASE: &str = "https://api.kilo.ai/api/gateway";
const CHAT: &str = "https://api.kilo.ai/api/gateway/chat/completions";
const CHAT_PATH: &str = "/chat/completions";

fn invalid() -> GatewayError {
    GatewayError::bad_request(
        "Kilo Free only supports anonymous chat on kilo-auto/free at the official endpoint.",
    )
    .with_code("kilo_free_configuration")
}

pub fn owns_payload(payload: &ProviderAccountPayload) -> bool {
    payload.auth_mode.as_deref() == Some(AUTH_MODE)
}

pub fn validate_declaration(cfg: &ProviderConfigYaml) -> Result<(), GatewayError> {
    if cfg.preset.as_deref() != Some(PRESET) && cfg.auth_mode.as_deref() != Some(AUTH_MODE) {
        return Ok(());
    }
    if cfg.preset.as_deref() != Some(PRESET)
        || !cfg.uses_provider_default_account()
        || !cfg.headers.is_empty()
        || !cfg.extra_body.is_empty()
        || cfg.keepalive.is_some()
        || cfg
            .auth_mode
            .as_deref()
            .is_some_and(|mode| mode != AUTH_MODE)
        || cfg.supported_models.as_slice() != [MODEL]
    {
        return Err(invalid());
    }
    Ok(())
}

pub fn validate(payload: &ProviderAccountPayload, adapter: &str) -> Result<(), GatewayError> {
    if !owns_payload(payload) {
        return Ok(());
    }
    if !cfg!(feature = "line-kilo-free") {
        return Err(crate::implementation_lines::compiled_out_error_for_line(
            crate::implementation_lines::RefactoredImplementationLine::KiloFree,
        ));
    }
    if adapter != "openai_compatible"
        || payload.base_url != BASE
        || !payload.api_key.is_empty()
        || payload.auth_token.is_some()
        || payload.session_auth.is_some()
        || payload.default_model.as_deref() != Some(MODEL)
        || payload.chat_completions_path.as_deref() != Some(CHAT_PATH)
        || payload.responses_path.is_some()
        || payload.request_client_profile.is_some()
        || payload.endpoint_execution_modes.is_some()
        || payload
            .execution_mode
            .is_some_and(|m| m != crate::routing::candidate::ProviderExecutionMode::DirectHttp)
    {
        return Err(invalid());
    }
    Ok(())
}

pub fn finalize_headers(
    payload: &ProviderAccountPayload,
    plan: &RequestPlan,
    headers: &mut HeaderMap,
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
    if plan.method != rquest::Method::POST
        || plan.url != CHAT
        || !plan.query.is_empty()
        || body.get("model").and_then(serde_json::Value::as_str) != Some(MODEL)
        || body.keys().any(|key| {
            !matches!(
                key.as_str(),
                "model"
                    | "messages"
                    | "stream"
                    | "stream_options"
                    | "max_tokens"
                    | "max_completion_tokens"
                    | "temperature"
                    | "top_p"
                    | "stop"
                    | "seed"
                    | "frequency_penalty"
                    | "presence_penalty"
                    | "tools"
                    | "tool_choice"
                    | "parallel_tool_calls"
                    | "response_format"
                    | "reasoning_effort"
                    | "n"
                    | "logprobs"
                    | "top_logprobs"
            )
        })
    {
        return Err(invalid());
    }
    // Neither caller auth nor process-wide client defaults may reach the anonymous endpoint.
    headers.clear();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    let accept = if body.get("stream").and_then(serde_json::Value::as_bool) == Some(true) {
        "text/event-stream"
    } else {
        "application/json"
    };
    headers.insert(ACCEPT, HeaderValue::from_static(accept));
    Ok(())
}

pub fn rate_limit(headers: &HeaderMap) -> GatewayError {
    let delay = retry_after_from_headers(headers, std::time::SystemTime::now());
    let reason = "Kilo free requests share the upstream IP limit.";
    let mut error = GatewayError::rate_limited(reason, delay.unwrap_or_default())
        .with_code("kilo_free_rate_limited")
        .with_retry_after_ms(delay);
    error.retryable = false;
    error.fallback_hint = FallbackHint::Abort {
        reason: reason.into(),
    };
    error
}

#[cfg(all(test, feature = "line-kilo-free"))]
mod tests;
