//! Express is the global publisher-model Key product, not AI Studio or project/location OAuth.
use crate::{
    error::GatewayError,
    routing::candidate::{ProviderAccountPayload, ProviderExecutionMode},
    upstream::common::RequestPlan,
};

pub const PRESET_ID: &str = "vertex-express-api";
pub const PROFILE: &str = "vertex_express_api";
pub const AUTH_MODE: &str = "vertex_express_api_key";
pub const BASE_URL: &str = "https://aiplatform.googleapis.com/v1/publishers/google";
pub const ADAPTER: &str = "gemini_api_compatible";

pub fn owns_base(base: &str) -> bool {
    // Only the project-less publisher prefix belongs here; legacy full Vertex is untouched.
    url::Url::parse(base).is_ok_and(|url| {
        url.host_str() == Some("aiplatform.googleapis.com")
            && ["/v1/publishers/", "/v1beta1/publishers/"]
                .iter()
                .any(|prefix| url.path().starts_with(prefix))
    })
}
pub fn owns_payload(payload: &ProviderAccountPayload) -> bool {
    payload.auth_mode.as_deref() == Some(AUTH_MODE) || owns_base(&payload.base_url)
}
pub fn invalid() -> GatewayError {
    GatewayError::bad_request("Invalid Vertex Express API Key product configuration.")
        .with_code("invalid_vertex_express_api_key")
}
pub fn valid_key(value: &str) -> bool {
    !value.is_empty() && value.len() <= 32_768 && value.bytes().all(|b| b.is_ascii_graphic())
}
pub fn valid_model(value: &str) -> bool {
    // A single resource segment. Do not accept URL encoding, full resources, or wildcard discovery.
    value.len() <= crate::provider_model_id::MAX_MODEL_BYTES
        && value
            .bytes()
            .next()
            .is_some_and(|b| b.is_ascii_alphanumeric())
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
}
pub fn validate(payload: &ProviderAccountPayload, adapter: &str) -> Result<(), GatewayError> {
    if !owns_payload(payload) {
        return Ok(());
    }
    if !cfg!(feature = "line-google-agent-platform-official") {
        return Err(
            GatewayError::conflict("Vertex Express transport was compiled out.")
                .with_code("gateway_provider_line_compiled_out"),
        );
    }
    if payload.base_url != BASE_URL && payload.base_url != format!("{BASE_URL}/")
        || adapter != ADAPTER
        || payload.adapter != ADAPTER
        || payload.auth_mode.as_deref() != Some(AUTH_MODE)
        || payload.auth_header_name.as_deref() != Some("x-goog-api-key")
        || payload.auth_token.is_some()
        || payload.session_auth.is_some()
        || payload.keepalive.is_some()
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
            &payload.responses_path,
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
        || payload
            .default_model
            .as_deref()
            .is_some_and(|model| !valid_model(model))
        || !payload.api_key.is_empty() && !valid_key(&payload.api_key)
    {
        return Err(invalid());
    }
    Ok(())
}
pub fn validate_plan(
    payload: &ProviderAccountPayload,
    plan: &RequestPlan,
) -> Result<(), GatewayError> {
    if !owns_payload(payload) {
        return Ok(());
    }
    validate(payload, payload.canonical_adapter())?;
    let resource = plan
        .url
        .strip_prefix(BASE_URL)
        .and_then(|p| p.strip_prefix("/models/"))
        .ok_or_else(invalid)?;
    let (model, stream) = if let Some(model) = resource.strip_suffix(":streamGenerateContent") {
        (model, true)
    } else {
        (
            resource
                .strip_suffix(":generateContent")
                .ok_or_else(invalid)?,
            false,
        )
    };
    if !valid_key(&payload.api_key)
        || !valid_model(model)
        // Until canonical multi-choice is implemented, reject caller-requested fan-out before spending upstream quota.
        || plan.body.as_ref().and_then(|body| body.pointer("/generationConfig/candidateCount"))
            .is_some_and(|count| count.as_u64() != Some(1))
        || plan.method != rquest::Method::POST
        || plan.query != crate::protocol::gemini::api::default_query(stream)
        || !crate::protocol::gemini::api::supports_endpoint(plan.response_kind)
        || plan
            .body
            .as_ref()
            .and_then(|v| v.get("contents"))
            .is_none_or(|v| !v.is_array())
    {
        return Err(invalid());
    }
    Ok(())
}

pub fn validate_request(
    payload: &ProviderAccountPayload,
    req: &crate::protocol::canonical::CanonicalRelayRequest,
    model: &str,
) -> Result<(), GatewayError> {
    if req
        .raw_body
        .get(super::vertex_turn::HISTORY_FIELD)
        .is_some()
        && !owns_payload(payload)
        && !super::vertex_oauth::business::owns_payload(payload)
        && !(super::dim_oauth::business::owns_payload(payload)
            && payload.canonical_adapter() == "gemini_api_compatible")
    {
        return Err(GatewayError::bad_request(
            "Vertex assistant state cannot target a non-Vertex product.",
        )
        .with_code("vertex_express_signed_history_wrong_product"));
    }
    if owns_payload(payload)
        && (!crate::protocol::gemini::api::supports_endpoint(req.endpoint_kind)
            || !valid_model(model))
    {
        return Err(invalid());
    }
    Ok(())
}

#[cfg(test)]
mod keep_tests;
#[cfg(test)]
mod tests;
