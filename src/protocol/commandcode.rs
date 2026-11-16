//! Command Code Provider API 的目录声明驱动协议通道；Go 套餐不是 API 产品。
use crate::protocol::{call_flow::*, canonical::EndpointKind};
use crate::{
    error::GatewayError, routing::candidate::ProviderAccountPayload, upstream::common::RequestPlan,
};

pub const PRESET: &str = "commandcode-api";
pub const PROFILE: &str = "commandcode_chat";
pub const AUTH_MODE: &str = "commandcode_api_key";
pub const BASE: &str = "https://api.commandcode.ai/provider/v1";
pub const SOURCE: &str = "https://api.commandcode.ai/provider/v1/models";

pub fn owns_payload(payload: &ProviderAccountPayload) -> bool {
    payload.auth_mode.as_deref() == Some(AUTH_MODE)
}
pub(crate) fn invalid() -> GatewayError {
    GatewayError::bad_request(
        "Invalid Command Code Provider API Key configuration or unsupported endpoint.",
    )
    .with_code("invalid_commandcode_chat")
}
pub fn validate(payload: &ProviderAccountPayload, adapter: &str) -> Result<(), GatewayError> {
    if !owns_payload(payload) {
        return Ok(());
    }
    crate::implementation_lines::ensure_payload_compiled(payload)?;
    if payload.base_url.trim_end_matches('/') != BASE
        || adapter != "openai_compatible"
        || (!payload.api_key.is_empty() && !valid_key(&payload.api_key))
        || payload.adapter != adapter
        || payload.auth_header_name.is_some()
        || payload.auth_token.is_some()
        || payload.session_auth.is_some()
        || payload.keepalive.is_some()
        || zdr::configured(&payload.headers).is_err()
        || catalog::read(payload).is_err()
        || !payload.discovered_protocols.is_empty()
        || payload
            .execution_mode
            .is_some_and(|m| m != crate::routing::candidate::ProviderExecutionMode::DirectHttp)
        || payload.endpoint_execution_modes.as_ref().is_some_and(|m| {
            m.values()
                .any(|v| *v != crate::routing::candidate::ProviderExecutionMode::DirectHttp)
        })
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
        || payload
            .chat_completions_path
            .as_deref()
            .is_some_and(|p| p != "/chat/completions")
        || [
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
        || payload.responses_path.as_deref().is_some_and(|p| {
            p != "/responses"
                || payload.chat_completions_path.is_some()
                || payload.messages_path.is_some()
                || catalog::read(payload).ok().flatten().is_none()
        })
        || payload.messages_path.as_deref().is_some_and(|p| {
            p != "/messages"
                || payload.chat_completions_path.is_some()
                || payload.responses_path.is_some()
                || catalog::read(payload).ok().flatten().is_none()
        })
    {
        return Err(invalid());
    }
    Ok(())
}
pub(crate) fn validate_declaration(
    cfg: &crate::routing::config::ProviderConfigYaml,
) -> anyhow::Result<()> {
    if cfg.preset.as_deref() != Some(PRESET)
        && cfg.auth_mode.as_deref() != Some(AUTH_MODE)
        && !cfg
            .credentials
            .iter()
            .any(|c| c.auth_mode.as_deref() == Some(AUTH_MODE))
    {
        return Ok(());
    }
    anyhow::ensure!(
        cfg.base_url.trim_end_matches('/') == BASE
            && zdr::configured(&cfg.headers).is_ok()
            && cfg
                .credentials
                .iter()
                .all(|c| zdr::configured(&c.headers).is_ok())
            && cfg.extra_body.is_empty()
            && cfg.credentials.iter().all(|c| c.extra_body.is_empty())
            && cfg.auth_mode.as_deref().is_none_or(|m| m == AUTH_MODE)
            && cfg.credentials.iter().all(|c| c.refresh_token.is_none()
                && c.discovery.is_none()
                && c.discovery_job.is_none()
                && c.auth_mode.as_deref().is_none_or(|m| m == AUTH_MODE)
                && c.base_url
                    .as_deref()
                    .is_none_or(|b| b.trim_end_matches('/') == BASE)),
        "Command Code Provider API Key identity and target cannot be overridden."
    );
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
    let body = plan.body.as_ref().ok_or_else(invalid)?;
    let path = match plan.response_kind {
        EndpointKind::ChatCompletions
            if catalog::supports(
                payload,
                body["model"].as_str().unwrap_or_default(),
                "/chat/completions",
            )? =>
        {
            "/chat/completions"
        }
        EndpointKind::Responses
            if catalog::supports_responses(
                payload,
                body["model"].as_str().unwrap_or_default(),
            )? =>
        {
            "/responses"
        }
        EndpointKind::Messages
            if catalog::supports(
                payload,
                body["model"].as_str().unwrap_or_default(),
                "/messages",
            )? =>
        {
            "/messages"
        }
        EndpointKind::SystemOne
            if catalog::supports(
                payload,
                body["model"].as_str().unwrap_or_default(),
                "/systemone",
            )? =>
        {
            "/systemone"
        }
        _ => return Err(invalid()),
    };
    if plan.url != format!("{BASE}{path}")
        || plan.method != rquest::Method::POST
        || !plan.query.is_empty()
        || !valid_key(&payload.api_key)
        || !body["model"].as_str().is_some_and(|id| !id.is_empty())
    {
        return Err(invalid());
    }
    zdr::validate_tools(payload, plan)
}
fn valid_key(key: &str) -> bool {
    !key.is_empty() && key.len() <= 16_384 && key.bytes().all(|b| b.is_ascii_graphic())
}
pub(crate) async fn read_response(
    response: rquest::Response,
) -> Result<crate::protocol::canonical::CanonicalRelayResponse, GatewayError> {
    let bytes =
        crate::protocol::upstream_body::collect_bounded_upstream_body(response, "CommandCode Chat")
            .await
            .map_err(|_| response_error())?;
    let body = serde_json::from_slice(&bytes).map_err(|_| response_error())?;
    crate::protocol::openai::chat_admission::validate(&body).map_err(|_| response_error())?;
    crate::protocol::openai::unpack_native_chat_response(&body).map_err(|_| response_error())
}
pub(crate) fn response_error() -> GatewayError {
    let mut error =
        GatewayError::server_error("CommandCode returned a failed or invalid response.")
            .with_code("commandcode_response_failed");
    error.http_status = Some(502);
    error.retryable = false;
    error.fallback_hint = crate::error::FallbackHint::Abort {
        reason: "CommandCode response received; do not replay inference.".into(),
    };
    error
}
pub(crate) fn stream_error() -> crate::protocol::stream_error::ProtocolStreamError {
    crate::protocol::stream_error::ProtocolStreamError::classified(
        "invalid_commandcode_stream: incomplete or invalid Chat stream",
        response_error(),
    )
}
pub fn finalize_headers(
    payload: &ProviderAccountPayload,
    plan: &RequestPlan,
    headers: &mut rquest::header::HeaderMap,
) -> Result<(), GatewayError> {
    if !owns_payload(payload) {
        return Ok(());
    }
    validate_plan(payload, plan)?;
    // 只发送本账号 Key；不保留 caller、代理或 SDK 注入的其他身份。
    headers.clear();
    if zdr::configured(&payload.headers)? {
        headers.insert(zdr::HEADER, rquest::header::HeaderValue::from_static("1"));
    }
    headers.insert(
        "authorization",
        rquest::header::HeaderValue::from_str(&format!("Bearer {}", payload.api_key))
            .map_err(|_| invalid())?,
    );
    headers.insert(
        "content-type",
        rquest::header::HeaderValue::from_static("application/json"),
    );
    headers.insert(
        "user-agent",
        rquest::header::HeaderValue::from_static("NeuroGateway/CommandCode"),
    );
    headers.insert(
        "accept",
        rquest::header::HeaderValue::from_static(
            if plan.body.as_ref().is_some_and(|b| b["stream"] == true) {
                "text/event-stream"
            } else {
                "application/json"
            },
        ),
    );
    Ok(())
}

pub(crate) mod catalog;
pub(crate) mod messages;
pub(crate) mod messages_stream;
#[cfg(test)]
mod messages_stream_extended_tests;
#[cfg(test)]
mod messages_stream_tests;
pub(crate) mod native_json;
mod native_request;
mod native_request_history;
#[cfg(all(test, feature = "line-commandcode-provider-api"))]
mod native_request_tests;
pub(crate) mod probe;
mod request;
mod responses;
pub(crate) mod responses_stream;
pub mod runtime;
pub(crate) mod systemone;
mod systemone_probe;
mod systemone_request;
mod systemone_response;
#[cfg(all(test, feature = "line-commandcode-provider-api"))]
mod systemone_tests;
#[cfg(test)]
pub(crate) mod test_support;
#[cfg(all(test, feature = "line-commandcode-provider-api"))]
mod tests;
#[cfg(all(test, feature = "line-commandcode-provider-api"))]
mod wire_tests;
pub(crate) mod zdr;
#[cfg(all(test, feature = "line-commandcode-provider-api"))]
mod zdr_tests;
