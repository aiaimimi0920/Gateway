//! 国际站显式 API Key 的文本通道；编码、流、取消和结算仍由共享 owner 负责。
use crate::protocol::{
    call_flow::*,
    canonical::{CanonicalRelayRequest, EndpointKind},
};
use crate::{
    error::GatewayError, routing::candidate::ProviderAccountPayload, upstream::common::RequestPlan,
};

pub const PRESET: &str = "agnes-api";
pub const PROFILE: &str = "agnes_api";
pub const AUTH_MODE: &str = "agnes_api_key";
pub const BASE: &str = "https://apihub.agnes-ai.com/v1";
pub const SOURCE: &str = "https://apihub.agnes-ai.com/v1/models";

pub fn owns_payload(payload: &ProviderAccountPayload) -> bool {
    payload.auth_mode.as_deref() == Some(AUTH_MODE)
}
fn invalid() -> GatewayError {
    GatewayError::bad_request(
        "Invalid Agnes international API Key configuration or unsupported endpoint.",
    )
    .with_code("invalid_agnes_api")
}
pub fn text_model(id: &str) -> bool {
    // 官方 image/video 命名空间不是文本模型；不复制社区的静态过期模型黑名单。
    let id = id.to_ascii_lowercase();
    !id.starts_with("agnes-image-") && !id.starts_with("agnes-video-")
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
        || !payload.headers.is_empty()
        || payload.extra_body.as_ref().is_some_and(|v| !v.is_empty())
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
        || payload
            .responses_path
            .as_deref()
            .is_some_and(|p| p != "/responses")
        || [
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
            && cfg.auth_mode.as_deref().is_none_or(|m| m == AUTH_MODE)
            && cfg.credentials.iter().all(|c| c.refresh_token.is_none()
                && c.auth_mode.as_deref().is_none_or(|m| m == AUTH_MODE)
                && c.base_url
                    .as_deref()
                    .is_none_or(|b| b.trim_end_matches('/') == BASE)),
        "Agnes international Key identity and target cannot be overridden."
    );
    Ok(())
}
pub fn validate_request(
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
) -> Result<(), GatewayError> {
    if !owns_payload(payload) {
        return Ok(());
    }
    // 本切片无远端会话管理或 hosted tools；不把状态请求降级为无状态文本成功。
    if !matches!(
        req.endpoint_kind,
        EndpointKind::ChatCompletions | EndpointKind::Messages | EndpointKind::Responses
    ) || req.previous_response_id.is_some()
        || req
            .raw_body
            .get("conversation")
            .is_some_and(|v| !v.is_null())
        || req.raw_body.get("store").is_some_and(|v| v == true)
        || req.raw_body.get("background").is_some_and(|v| v == true)
        || req.tools.iter().any(|tool| tool.tool_type != "function")
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
    let body = plan.body.as_ref().ok_or_else(invalid)?;
    let path = match plan.response_kind {
        EndpointKind::ChatCompletions => "/chat/completions",
        EndpointKind::Responses => "/responses",
        _ => return Err(invalid()),
    };
    if plan.url != format!("{BASE}{path}")
        || plan.method != rquest::Method::POST
        || !plan.query.is_empty()
        || !valid_key(&payload.api_key)
        || !body["model"]
            .as_str()
            .is_some_and(|id| !id.is_empty() && text_model(id))
    {
        return Err(invalid());
    }
    Ok(())
}
fn valid_key(key: &str) -> bool {
    !key.is_empty() && key.len() <= 16_384 && key.bytes().all(|b| b.is_ascii_graphic())
}
pub(crate) fn validate_json_response(
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    body: &serde_json::Value,
) -> Result<(), GatewayError> {
    if !owns_payload(payload) || req.endpoint_kind != EndpointKind::Responses {
        return Ok(());
    }
    if matches!(body["status"].as_str(), Some("completed" | "incomplete"))
        && body["error"].is_null()
        && body["output"].is_array()
    {
        return Ok(());
    }
    // HTTP 200 does not make a failed generation billable. It has already run,
    // so neither provider prose nor a failure envelope authorizes replay.
    let mut error =
        GatewayError::server_error("Agnes Responses returned a failed or invalid response.")
            .with_code("agnes_response_failed");
    error.http_status = Some(502);
    error.retryable = false;
    error.fallback_hint = crate::error::FallbackHint::Abort {
        reason: "Agnes response already received; do not replay inference.".into(),
    };
    Err(error)
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
        rquest::header::HeaderValue::from_static("NeuroGateway/Agnes"),
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

pub(crate) mod probe;
#[cfg(test)]
pub(crate) mod test_support;
#[cfg(all(test, feature = "line-agnes-official-api"))]
mod tests;
#[cfg(all(test, feature = "line-agnes-official-api"))]
mod wire_tests;
