//! Account-bound Workers AI policy; encoding, transport and stream handling stay shared.
use crate::{
    error::GatewayError, routing::candidate::ProviderAccountPayload, upstream::common::RequestPlan,
};

pub const PRESET: &str = "cloudflare-workers-ai";
pub const PROFILE: &str = "cloudflare_workers_ai";
pub const AUTH_MODE: &str = "cloudflare_workers_ai_token";
const PREFIX: &str = "https://api.cloudflare.com/client/v4/accounts/";

#[derive(Clone, Debug)]
pub struct Endpoint {
    pub base_url: String,
}

impl Endpoint {
    pub fn parse(base: &str) -> Option<Self> {
        let account = base
            .strip_prefix(PREFIX)?
            .trim_end_matches('/')
            .strip_suffix("/ai/v1")?;
        if account.len() != 32
            || !account
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return None;
        }
        Some(Self {
            base_url: format!("{PREFIX}{account}/ai/v1"),
        })
    }
    pub fn source_url(&self) -> String {
        format!(
            "{}/models/search?task=Text%20Generation&per_page=100&page=1",
            self.base_url.trim_end_matches("/v1")
        )
    }
}

pub fn owns_payload(payload: &ProviderAccountPayload) -> bool {
    payload.auth_mode.as_deref() == Some(AUTH_MODE) || Endpoint::parse(&payload.base_url).is_some()
}
pub fn invalid() -> GatewayError {
    GatewayError::bad_request(
        "Invalid Cloudflare Workers AI account target or token configuration.",
    )
    .with_code("invalid_cloudflare_workers_ai")
}
pub fn valid_key(key: &str) -> bool {
    !key.is_empty() && key.len() <= 16_384 && key.bytes().all(|b| b.is_ascii_graphic())
}
pub fn validate(payload: &ProviderAccountPayload, adapter: &str) -> Result<(), GatewayError> {
    if !owns_payload(payload) {
        return Ok(());
    }
    if !cfg!(feature = "line-cloudflare-workers-ai") {
        return Err(GatewayError::conflict(
            "Cloudflare Workers AI implementation line was compiled out.",
        )
        .with_code("gateway_provider_line_compiled_out"));
    }
    if Endpoint::parse(&payload.base_url).is_none()
        || adapter != "openai_compatible"
        || payload.adapter != adapter
        || !matches!(
            payload.auth_mode.as_deref(),
            None | Some("bearer") | Some(AUTH_MODE)
        )
        || payload.auth_header_name.is_some()
        || payload.auth_token.is_some()
        || payload.session_auth.is_some()
        || payload.keepalive.is_some()
        || !payload.discovered_protocols.is_empty()
        || payload.execution_mode.is_some_and(|mode| {
            mode != crate::routing::candidate::ProviderExecutionMode::DirectHttp
        })
        || payload
            .endpoint_execution_modes
            .as_ref()
            .is_some_and(|modes| {
                modes.values().any(|mode| {
                    *mode != crate::routing::candidate::ProviderExecutionMode::DirectHttp
                })
            })
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
    let base = Endpoint::parse(&payload.base_url).ok_or_else(invalid)?;
    let path = plan.url.strip_prefix(&base.base_url).ok_or_else(invalid)?;
    let body = plan.body.as_ref().ok_or_else(invalid)?;
    if !valid_key(&payload.api_key)
        || plan.method != rquest::Method::POST
        || !plan.query.is_empty()
        || !matches!(path, "/chat/completions" | "/responses" | "/embeddings")
        || !body["model"].is_string()
        || !matches!(
            plan.response_kind,
            crate::protocol::canonical::EndpointKind::ChatCompletions
                | crate::protocol::canonical::EndpointKind::Messages
                | crate::protocol::canonical::EndpointKind::Completions
                | crate::protocol::canonical::EndpointKind::Responses
                | crate::protocol::canonical::EndpointKind::Embeddings
        )
    {
        return Err(invalid());
    }
    if path == "/responses"
        && (!matches!(
            body["model"].as_str(),
            Some("@cf/openai/gpt-oss-120b" | "@cf/openai/gpt-oss-20b")
        ) || body["stream"] != false)
    {
        return Err(GatewayError::bad_request(
            "Cloudflare Responses requires a GPT-OSS model and stream:false.",
        )
        .with_code("unsupported_cloudflare_responses"));
    }
    Ok(())
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
    // The selected account owns authentication, including after caller/fingerprint header merging.
    for name in [
        "cookie",
        "host",
        "x-auth-key",
        "x-auth-email",
        "api-key",
        "x-api-key",
    ] {
        headers.remove(name);
    }
    headers.insert(
        rquest::header::AUTHORIZATION,
        rquest::header::HeaderValue::from_str(&format!("Bearer {}", payload.api_key))
            .map_err(|_| invalid())?,
    );
    Ok(())
}

#[cfg(test)]
#[path = "cloudflare_workers_ai_tests.rs"]
mod tests;
