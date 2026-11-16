//! Opt-in runtime-root OAuth wire; legacy REST and static Key identities remain separate.
use super::RuntimeTarget;
use crate::{
    error::GatewayError,
    routing::candidate::{ProviderAccountPayload, ProviderExecutionMode},
};

pub const AUTH_MODE: &str = "kiro_oauth_runtime";
pub fn owns_payload(payload: &ProviderAccountPayload) -> bool {
    payload.auth_mode.as_deref() == Some(AUTH_MODE)
}
pub fn invalid() -> GatewayError {
    GatewayError::bad_request("Invalid Kiro registered OAuth runtime configuration.")
        .with_code("invalid_kiro_oauth_runtime")
}
pub fn validate(payload: &ProviderAccountPayload) -> Result<(), GatewayError> {
    if !owns_payload(payload) {
        return Ok(());
    }
    if !cfg!(feature = "line-kiro-official-vendor-api") {
        return Err(
            GatewayError::conflict("Kiro OAuth runtime transport was compiled out.")
                .with_code("gateway_provider_line_compiled_out"),
        );
    }
    if RuntimeTarget::from_payload(payload).is_err()
        || payload.canonical_adapter() != "kiro_compatible"
        || payload.adapter != "kiro_compatible"
        || payload.responses_path.as_deref() != Some("/")
        || payload.auth_token.is_some()
        || payload.auth_header_name.is_some()
        || payload.keepalive.is_some()
        || !payload.headers.is_empty()
        || !payload.discovered_protocols.is_empty()
        || !payload.api_key.is_empty() && !super::business::valid_token(&payload.api_key)
        || payload.session_auth.as_ref().is_none_or(|s| {
            s.transport != "bearer"
                || s.header_name()
                    .is_some_and(|name| !name.eq_ignore_ascii_case("authorization"))
        })
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
        || payload.extra_body.as_ref().is_none_or(|body| {
            body.keys()
                .any(|name| !matches!(name.as_str(), "profileArn" | "kiroMachineId"))
        })
    {
        return Err(invalid());
    }
    Ok(())
}
pub fn validate_plan(
    payload: &ProviderAccountPayload,
    plan: &crate::upstream::common::RequestPlan,
) -> Result<(), GatewayError> {
    if !owns_payload(payload) {
        return Ok(());
    }
    validate(payload)?;
    let target = RuntimeTarget::from_payload(payload).map_err(|_| invalid())?;
    let body = plan
        .body
        .as_ref()
        .and_then(serde_json::Value::as_object)
        .ok_or_else(invalid)?;
    if !super::business::valid_token(&payload.api_key)
        || plan.method != rquest::Method::POST
        || plan.url != format!("{}/", target.base_url())
        || !plan.query.is_empty()
        || !crate::upstream::kiro::supports_endpoint(plan.response_kind)
        || body.len() != 2
        || body.get("profileArn").and_then(serde_json::Value::as_str) != Some(target.profile_arn())
        || body
            .get("conversationState")
            .is_none_or(|state| !crate::protocol::kiro::valid_runtime_state(state, payload))
    {
        return Err(invalid());
    }
    Ok(())
}

/// Hosted repair must bind the business target too, not merely the OIDC registration.
pub(crate) fn matches_cache(payload: Option<&serde_json::Value>, target: &RuntimeTarget) -> bool {
    let Some(payload) = payload.and_then(serde_json::Value::as_object) else {
        return false;
    };
    let modes: Vec<_> = ["auth_mode", "authMode"]
        .iter()
        .filter_map(|name| payload.get(*name))
        .collect();
    if modes.is_empty() || modes.iter().any(|mode| mode.as_str() != Some(AUTH_MODE)) {
        return false;
    }
    let bodies: Vec<_> = ["extra_body", "extraBody"]
        .iter()
        .filter_map(|name| payload.get(*name))
        .collect();
    !bodies.is_empty()
        && bodies.iter().all(|body| {
            body.get("profileArn").and_then(serde_json::Value::as_str) == Some(target.profile_arn())
        })
}
