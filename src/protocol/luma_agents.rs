//! Product-scoped contract for the official Agents API, independent of Luma Web.
mod media_ref;
mod request;
mod video;

use crate::{
    error::{ErrorKind, FallbackHint, GatewayError},
    routing::candidate::ProviderAccountPayload,
};
pub(crate) use request::{GenerationRequest, Media};

pub const ADAPTER: &str = "lumalabs_agents_compatible";
pub const PRESET_ID: &str = "lumalabs-api";
pub const PROFILE: &str = "lumalabs_agents_api";
pub const FAMILY: &str = "lumalabs_agents_media";
pub const BASE: &str = "https://agents.lumalabs.ai/v1";

/// Share closed native audit facts between public finalization and explicit media probes.
pub(crate) fn record_success_audit(
    adapter: Option<&str>,
    body: &serde_json::Value,
    trace: &mut serde_json::Value,
) -> Option<String> {
    if adapter != Some(ADAPTER) {
        return None;
    }
    let job = body.get("gateway_luma_generation")?;
    let id = job.get("id")?.as_str()?;
    // Only the selected product sender validates these facts; never copy URLs or arbitrary metadata.
    trace["lumaAgentsGeneration"] = serde_json::json!({"id":id,"model":job["model"],"type":job["type"],
        "state":job["state"],"usage":{"credits_charged":job["usage"]["credits_charged"]}});
    Some(id.to_owned())
}

/// Reference-based edits have no local upload; product validation follows auth and model resolution.
pub(crate) fn normalize_source_edit(
    body: serde_json::Value,
) -> Result<crate::protocol::canonical::CanonicalRelayRequest, GatewayError> {
    let mut request = crate::protocol::gemini_business::normalize_image_generations(body)?;
    request.endpoint_kind = crate::protocol::canonical::EndpointKind::ImagesEdits;
    Ok(request)
}

/// Ownership is deliberately broader than valid destinations, so malformed targets cannot escape.
pub fn owns_base(base: &str) -> bool {
    url::Url::parse(base).is_ok_and(|url| url.host_str() == Some("agents.lumalabs.ai"))
}
pub fn valid_key(key: &str) -> bool {
    key.starts_with("luma-api-")
        && key.len() > 9
        && key.len() <= 4096
        && key.bytes().all(|b| b.is_ascii_graphic())
}
pub fn valid_model(model: &str) -> bool {
    matches!(model, "uni-1" | "uni-1-max" | "ray-3.2")
}

/// Run before alternate executors can consume a known Agents key/target.
pub fn validate(payload: &ProviderAccountPayload, adapter: &str) -> Result<(), GatewayError> {
    let target = url::Url::parse(&payload.base_url).ok();
    let agents_origin = target
        .as_ref()
        .is_some_and(|u| u.host_str() == Some("agents.lumalabs.ai"));
    let agents_key = payload.api_key.starts_with("luma-api-");
    if payload.adapter != ADAPTER && adapter != ADAPTER && !agents_origin && !agents_key {
        return Ok(());
    }
    if adapter != ADAPTER
        || payload.adapter != ADAPTER
        || !target.is_some_and(|u| {
            u.scheme() == "https"
                && u.host_str() == Some("agents.lumalabs.ai")
                && u.port().is_none()
                && u.username().is_empty()
                && u.password().is_none()
                && u.query().is_none()
                && u.fragment().is_none()
                && matches!(u.path(), "/v1" | "/v1/")
        })
        || payload.session_auth.is_some()
        || payload.auth_token.is_some()
        || payload.auth_mode.as_deref().is_some_and(|m| m != "bearer")
    {
        return Err(input(
            "luma_agents_identity",
            "Luma Agents 必须使用独立 API adapter、固定官方目标和 API Key。",
        ));
    }
    Ok(())
}

pub(crate) fn input(code: &'static str, message: &'static str) -> GatewayError {
    failure(ErrorKind::BadRequest, 400, code, message)
}
pub(crate) fn failure(
    kind: ErrorKind,
    status: u16,
    code: &'static str,
    message: &'static str,
) -> GatewayError {
    let mut error = GatewayError::server_error(message)
        .with_code(code)
        .with_provider(ADAPTER);
    error.kind = kind;
    error.http_status = Some(status);
    error.retryable = false;
    error.fallback_hint = FallbackHint::Abort {
        reason: "Luma Agents 生成不自动重提或切换渠道。".into(),
    };
    error
}
pub(crate) fn invalid_request() -> GatewayError {
    input(
        "luma_agents_request",
        "Luma Agents 请求参数或组合不受支持；没有提交生成。",
    )
}

#[cfg(test)]
mod tests;
