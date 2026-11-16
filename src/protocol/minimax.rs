//! M Plan 显式订阅 Key；区域绑定和最终发送边界属于产品，协议与结算复用共享实现。
use crate::protocol::{
    call_flow::*,
    canonical::{CanonicalRelayRequest, EndpointKind},
};
use crate::{
    error::GatewayError, routing::candidate::ProviderAccountPayload, upstream::common::RequestPlan,
};

pub const PRESET: &str = "minimax-m-plan";
pub const PRESET_CN: &str = "minimax-m-plan-cn";
pub const AUTH_MODE: &str = "minimax_m_plan_key";
pub const BASE: &str = "https://api.minimax.io/anthropic";
pub const BASE_CN: &str = "https://api.minimax.cn/anthropic";
pub(crate) mod probe;
pub(crate) mod stream;

pub fn owns_payload(payload: &ProviderAccountPayload) -> bool {
    payload.auth_mode.as_deref() == Some(AUTH_MODE)
}
pub fn preset_base(preset: &str) -> Option<&'static str> {
    match preset {
        PRESET => Some(BASE),
        PRESET_CN => Some(BASE_CN),
        _ => None,
    }
}
fn valid_base(base: &str) -> bool {
    matches!(base.trim_end_matches('/'), BASE | BASE_CN)
}
fn invalid() -> GatewayError {
    GatewayError::bad_request("Invalid MiniMax M Plan Key configuration or unsupported request.")
        .with_code("invalid_minimax_m_plan")
}
fn valid_key(key: &str) -> bool {
    // 官方未发布可依赖的 Key 前缀/长度合同；只约束安全传输，不推断套餐权益。
    !key.is_empty() && key.len() <= 16_384 && key.bytes().all(|b| b.is_ascii_graphic())
}
pub fn validate(payload: &ProviderAccountPayload, adapter: &str) -> Result<(), GatewayError> {
    if !owns_payload(payload) {
        return Ok(());
    }
    crate::implementation_lines::ensure_payload_compiled(payload)?;
    if !valid_base(&payload.base_url)
        || adapter != "anthropic_compatible"
        || payload.adapter != adapter
        || (!payload.api_key.is_empty() && !valid_key(&payload.api_key))
        || payload.auth_header_name.is_some()
        || payload.auth_token.is_some()
        || payload.session_auth.is_some()
        || payload.keepalive.is_some()
        || !payload.headers.is_empty()
        || payload.beta_headers.is_some()
        || payload
            .anthropic_version
            .as_deref()
            .is_some_and(|v| v != "2023-06-01")
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
            !matches!(f.call_adapter, CallAdapter::Auto | CallAdapter::Anthropic)
                || f.fingerprint_adapter != FingerprintAdapter::Auto
                || !matches!(
                    f.special_adapter,
                    SpecialAdapter::Auto | SpecialAdapter::None
                )
        })
        || payload
            .messages_path
            .as_deref()
            .is_some_and(|p| p != "/v1/messages")
        || [
            &payload.chat_completions_path,
            &payload.responses_path,
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
    let expected = cfg.preset.as_deref().and_then(preset_base);
    if expected.is_none()
        && cfg.auth_mode.as_deref() != Some(AUTH_MODE)
        && !cfg
            .credentials
            .iter()
            .any(|c| c.auth_mode.as_deref() == Some(AUTH_MODE))
    {
        return Ok(());
    }
    let base = cfg.base_url.trim_end_matches('/');
    anyhow::ensure!(
        valid_base(base)
            && expected.is_none_or(|b| b == base)
            && cfg.auth_mode.as_deref().is_none_or(|m| m == AUTH_MODE)
            && cfg.credentials.iter().all(|c| c.refresh_token.is_none()
                && c.auth_mode.as_deref().is_none_or(|m| m == AUTH_MODE)
                && c.base_url
                    .as_deref()
                    .is_none_or(|b| b.trim_end_matches('/') == base)),
        "MiniMax M Plan identity and regional target cannot be overridden."
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
        || req.tools.iter().any(|t| t.tool_type != "function")
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
    if plan.url != format!("{}/v1/messages", payload.base_url.trim_end_matches('/'))
        || plan.method != rquest::Method::POST || !plan.query.is_empty() || plan.response_kind != EndpointKind::Messages
        || !valid_key(&payload.api_key) || !body["model"].as_str().is_some_and(|m| !m.trim().is_empty())
        // 当前官方只支持 auto；拒绝无法兑现的工具策略，不静默改成 auto。
        || body.get("tool_choice").is_some_and(|c| !c.is_null() && c["type"] != "auto")
    {
        return Err(invalid());
    }
    Ok(())
}
pub fn prepare_plan(
    payload: &ProviderAccountPayload,
    plan: &mut RequestPlan,
) -> Result<(), GatewayError> {
    if owns_payload(payload) {
        // 共享 canonical normalizer 将 Anthropic auto 转为字符串；恢复官方 wire 形状。
        if let Some(body) = plan.body.as_mut() {
            if body["tool_choice"] == "auto" {
                body["tool_choice"] = serde_json::json!({"type":"auto"});
            }
        }
    }
    validate_plan(payload, plan)
}
pub(crate) async fn read_json(
    response: rquest::Response,
) -> Result<serde_json::Value, GatewayError> {
    let bytes =
        crate::protocol::upstream_body::collect_bounded_upstream_body(response, "MiniMax Messages")
            .await
            .map_err(|_| response_error())?;
    serde_json::from_slice(&bytes).map_err(|_| response_error())
}
pub(crate) fn validate_json_response(
    payload: &ProviderAccountPayload,
    body: &serde_json::Value,
) -> Result<(), GatewayError> {
    if !owns_payload(payload) {
        return Ok(());
    }
    if body["type"] == "message"
        && body["role"] == "assistant"
        && body["content"].is_array()
        && body["error"].is_null()
        && body["stop_reason"].as_str().is_some_and(|v| !v.is_empty())
    {
        return Ok(());
    }
    Err(response_error())
}
fn response_error() -> GatewayError {
    // 已收到生成响应，未知结构不允许触发另一次推理或被结算为空成功。
    let mut error =
        GatewayError::server_error("MiniMax returned a failed or invalid Messages response.")
            .with_code("minimax_response_failed");
    error.http_status = Some(502);
    error.retryable = false;
    error.fallback_hint = crate::error::FallbackHint::Abort {
        reason: "MiniMax response received; do not replay inference.".into(),
    };
    error
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
    // 官方推荐单一 Bearer；清理 caller/SDK 的第二身份后禁用 ambient headers 和 redirect。
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
        rquest::header::HeaderValue::from_static("NeuroGateway/MiniMax"),
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

#[cfg(test)]
pub(crate) mod test_support;
#[cfg(all(test, feature = "line-minimax-m-plan"))]
mod tests;
#[cfg(all(test, feature = "line-minimax-m-plan"))]
mod wire_tests;
