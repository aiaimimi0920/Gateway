//! Typed Full Vertex is opt-in; old Gemini/Vertex aliases keep their legacy behavior.
use super::{AuthorizationContext, VertexTarget};
use crate::{
    error::GatewayError,
    routing::candidate::{ProviderAccountPayload, ProviderExecutionMode},
    upstream::common::RequestPlan,
};

pub const AUTH_MODE: &str = "vertex_user_oauth";
pub fn owns_payload(payload: &ProviderAccountPayload) -> bool {
    payload.auth_mode.as_deref() == Some(AUTH_MODE)
}
pub fn invalid() -> GatewayError {
    GatewayError::bad_request("Invalid Full Vertex user OAuth configuration.")
        .with_code("invalid_vertex_user_oauth")
}
pub fn valid_token(value: &str) -> bool {
    super::valid_atom(value, 32_768)
}

/// Reconstruct only the fixed publisher target, never a caller-selected origin/path.
pub fn target(payload: &ProviderAccountPayload) -> Result<VertexTarget, GatewayError> {
    let url = url::Url::parse(&payload.base_url).map_err(|_| invalid())?;
    let parts: Vec<_> = url.path().split('/').collect();
    let ["", "v1", "projects", project, "locations", location, "publishers", "google"] =
        parts.as_slice()
    else {
        return Err(invalid());
    };
    let quota = payload
        .headers
        .get("x-goog-user-project")
        .map(String::as_str);
    let target = VertexTarget::new(project, location, quota).map_err(|_| invalid())?;
    if target.base_url().map_err(|_| invalid())? != payload.base_url {
        return Err(invalid());
    }
    Ok(target)
}
pub fn validate(payload: &ProviderAccountPayload, adapter: &str) -> Result<(), GatewayError> {
    if !owns_payload(payload) {
        return Ok(());
    }
    if !cfg!(feature = "line-google-agent-platform-official") {
        return Err(
            GatewayError::conflict("Full Vertex OAuth transport was compiled out.")
                .with_code("gateway_provider_line_compiled_out"),
        );
    }
    target(payload)?;
    if adapter != "gemini_api_compatible"
        || payload.adapter != adapter
        || payload.auth_header_name.as_deref() != Some("authorization")
        || payload.auth_token.is_some()
        || payload.session_auth.is_some()
        || payload.keepalive.is_some()
        || payload
            .headers
            .keys()
            .any(|key| key != "x-goog-user-project")
        || payload
            .extra_body
            .as_ref()
            .is_some_and(|extra| !extra.is_empty())
        || !payload.discovered_protocols.is_empty()
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
            .is_some_and(|m| m.values().any(|m| *m != ProviderExecutionMode::DirectHttp))
        || [
            &payload.responses_path,
            &payload.chat_completions_path,
            &payload.completions_path,
            &payload.messages_path,
            &payload.embeddings_path,
            &payload.audio_transcriptions_path,
            &payload.audio_speech_path,
            &payload.search_path,
            &payload.fetch_path,
            &payload.research_path,
            &payload.balance_path,
        ]
        .iter()
        .any(|path| path.is_some())
        || payload
            .default_model
            .as_deref()
            .is_some_and(|m| !crate::protocol::vertex_express::valid_model(m))
        || !payload.api_key.is_empty() && !valid_token(&payload.api_key)
    {
        return Err(invalid());
    }
    Ok(())
}
pub fn validate_context(
    payload: &ProviderAccountPayload,
    context: &AuthorizationContext,
) -> Result<(), GatewayError> {
    context.validate().map_err(|_| invalid())?;
    if !owns_payload(payload) || &target(payload)? != context.target() {
        return Err(invalid());
    }
    validate(payload, payload.canonical_adapter())
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
        .strip_prefix(&payload.base_url)
        .and_then(|s| s.strip_prefix("/models/"))
        .ok_or_else(invalid)?;
    let (model, stream) = if let Some(m) = resource.strip_suffix(":streamGenerateContent") {
        (m, true)
    } else {
        (
            resource
                .strip_suffix(":generateContent")
                .ok_or_else(invalid)?,
            false,
        )
    };
    if !valid_token(&payload.api_key)
        || !crate::protocol::vertex_express::valid_model(model)
        || plan.method != rquest::Method::POST
        || plan.query != crate::protocol::gemini::api::default_query(stream)
        || plan
            .body
            .as_ref()
            .and_then(|v| v.pointer("/generationConfig/candidateCount"))
            .is_some_and(|count| count.as_u64() != Some(1))
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
    if owns_payload(payload)
        && (!crate::protocol::gemini::api::supports_endpoint(req.endpoint_kind)
            || !crate::protocol::vertex_express::valid_model(model))
    {
        return Err(invalid());
    }
    Ok(())
}
