//! Typed Nous is opt-in. Legacy generic endpoint records retain their existing identity and contract.
use super::{AuthorizationContext, OAuthClient};
use crate::{
    error::GatewayError,
    routing::candidate::{ProviderAccountPayload, ProviderExecutionMode},
};
pub const AUTH_MODE: &str = "nous_portal_oauth";
pub fn owns_payload(payload: &ProviderAccountPayload) -> bool {
    payload.auth_mode.as_deref() == Some(AUTH_MODE)
}
pub fn invalid() -> GatewayError {
    GatewayError::bad_request("Invalid Nous Portal OAuth product configuration.")
        .with_code("invalid_nous_portal_oauth")
}
pub fn validate_context(
    payload: &ProviderAccountPayload,
    context: &AuthorizationContext,
) -> Result<(), GatewayError> {
    let client = OAuthClient::new(context.client_id()).map_err(|_| invalid())?;
    context
        .validate_binding(&payload.api_key, &client)
        .map_err(|_| invalid())?;
    validate_shape(payload, payload.canonical_adapter())?;
    if payload.base_url.trim_end_matches('/') != context.inference_target().base_url()
        || !payload
            .expires_at
            .as_deref()
            .is_some_and(|s| context.matches_expiry(s))
    {
        return Err(invalid());
    }
    Ok(())
}

/// Shared shape contract: compilation admits expired material for its refresh owner, not dispatch.
pub(crate) fn validate_shape(
    payload: &ProviderAccountPayload,
    adapter: &str,
) -> Result<(), GatewayError> {
    if !owns_payload(payload)
        || adapter != "openai_compatible"
        || payload.adapter != "openai_compatible"
        || super::InferenceTarget::from_base_url(&payload.base_url).is_err()
        || !payload.api_key.is_empty() && !super::atom(&payload.api_key, 32_768)
        || payload.auth_token.is_some()
        || payload.session_auth.is_some()
        || payload.keepalive.is_some()
        || payload
            .auth_header_name
            .as_deref()
            .is_some_and(|s| s != "authorization")
        || !payload.headers.is_empty()
        || payload.extra_body.as_ref().is_some_and(|v| !v.is_empty())
        || !payload.discovered_protocols.is_empty()
        || payload
            .execution_mode
            .is_some_and(|m| m != ProviderExecutionMode::DirectHttp)
        || payload
            .endpoint_execution_modes
            .as_ref()
            .is_some_and(|m| m.values().any(|m| *m != ProviderExecutionMode::DirectHttp))
        || !matches!(
            payload.request_client_profile,
            None | Some(crate::protocol::request_profile::RequestClientProfile::Auto)
        )
        || payload.call_flow.is_some_and(|flow| {
            use crate::protocol::call_flow::*;
            !matches!(flow.call_adapter, CallAdapter::Auto | CallAdapter::Openai)
                || flow.fingerprint_adapter != FingerprintAdapter::Auto
                || !matches!(
                    flow.special_adapter,
                    SpecialAdapter::Auto | SpecialAdapter::None
                )
        })
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
        .any(|p| p.is_some())
    {
        return Err(invalid());
    }
    Ok(())
}
