//! Directory and virtual selections share ordinary Chat/Messages personal admission.
use super::management_boundary::BASE;
use crate::{error::GatewayError, routing::candidate::ProviderAccountPayload};
pub(crate) fn owns_payload(payload: &ProviderAccountPayload) -> bool {
    payload.auth_mode.as_deref() == Some(super::AUTH_MODE)
}
pub(crate) fn invalid() -> GatewayError {
    GatewayError::bad_request(
        "LobsterAI requires a declared supported model and a supported text/function request",
    )
    .with_code("lobsterai_inference_unavailable")
}
pub(crate) fn validate(payload: &ProviderAccountPayload) -> Result<(), GatewayError> {
    use crate::protocol::call_flow::{CallAdapter, FingerprintAdapter, SpecialAdapter};
    use crate::routing::candidate::ProviderExecutionMode::DirectHttp;
    if !owns_payload(payload)
        || payload.base_url != BASE
        || payload.adapter != "openai_compatible"
        || !super::atom(&payload.api_key, 32768)
        || payload.auth_token.is_some()
        || payload.auth_header_name.is_some()
        || payload.session_auth.is_some()
        || payload.keepalive.is_some()
        || payload.expires_at.is_some()
        || !payload.headers.is_empty()
        || !payload.discovered_protocols.is_empty()
        || payload.execution_mode.is_some_and(|m| m != DirectHttp)
        || payload
            .endpoint_execution_modes
            .as_ref()
            .is_some_and(|m| m.values().any(|v| *v != DirectHttp))
        || !matches!(
            payload.request_client_profile,
            None | Some(crate::protocol::request_profile::RequestClientProfile::Auto)
        )
        || payload.call_flow.is_some_and(|f| {
            !matches!(f.call_adapter, CallAdapter::Auto | CallAdapter::Openai)
                || f.fingerprint_adapter != FingerprintAdapter::Auto
                || !matches!(
                    f.special_adapter,
                    SpecialAdapter::Auto | SpecialAdapter::None
                )
        })
        || [
            &payload.chat_completions_path,
            &payload.responses_path,
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
    {
        return Err(invalid());
    }
    super::metadata::read(payload)?;
    Ok(())
}
pub(crate) fn model(
    payload: &ProviderAccountPayload,
    id: &str,
    req: &crate::protocol::canonical::CanonicalRelayRequest,
) -> Result<super::DeclaredModel, GatewayError> {
    crate::protocol::openai::text_tools_request::validate(req, invalid)?;
    let metadata = super::metadata::read(payload)?;
    let model = metadata
        .catalog
        .as_ref()
        .and_then(|c| c.models().iter().find(|m| m.model_id == id))
        .ok_or_else(invalid)?;
    if super::virtual_model::is_virtual(id) {
        if model.accessible == Some(false) {
            return Err(invalid());
        }
        if let Some(selection) = metadata.selection {
            if selection.preset != id {
                return Err(invalid());
            }
            concrete(&selection.model, req)?;
            return Ok(selection.model);
        }
        // Synchronous cash admission needs output limits without performing network IO.
        return Ok(model.clone());
    }
    concrete(model, req)?;
    Ok(model.clone())
}
pub(super) fn concrete(
    model: &super::DeclaredModel,
    req: &crate::protocol::canonical::CanonicalRelayRequest,
) -> Result<(), GatewayError> {
    let kimi = |s: &str| {
        let id: String = s
            .chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .flat_map(char::to_lowercase)
            .collect();
        matches!(id.as_str(), "kimik3" | "kimik3youdaoinner")
    };
    if !matches!(model.api_format.as_deref(), Some("openai" | "anthropic"))
        || model.runtime_profile.is_some()
        || model.accessible == Some(false)
        || super::virtual_model::is_virtual(&model.model_id)
        || kimi(&model.model_id)
        || model.model_name.as_deref().is_some_and(kimi)
        || (model.supports_tool_calling != Some(true)
            && (!req.tools.is_empty()
                || req.messages.iter().any(|m| {
                    !m.tool_calls.is_empty()
                        || m.role == crate::protocol::canonical::MessageRole::Tool
                })))
    {
        return Err(invalid());
    }
    Ok(())
}
pub(crate) fn response_error() -> GatewayError {
    let mut error =
        GatewayError::server_error("LobsterAI returned a failed or invalid Chat response")
            .with_code("lobsterai_response_failed");
    error.http_status = Some(502);
    error.retryable = false;
    error.fallback_hint = crate::error::FallbackHint::Abort {
        reason: "LobsterAI response received; do not replay inference".into(),
    };
    error
}
pub(crate) fn stream_error() -> crate::protocol::stream_error::ProtocolStreamError {
    crate::protocol::stream_error::ProtocolStreamError::classified(
        "invalid_lobsterai_stream: incomplete or invalid Chat stream",
        response_error(),
    )
}
