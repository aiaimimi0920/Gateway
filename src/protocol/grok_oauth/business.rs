//! Build bearer material stays on its fixed Responses audience, even after late configuration merges.
use super::BUSINESS_BASE_URL;
use crate::{
    error::GatewayError,
    protocol::{
        call_flow::{CallAdapter, FingerprintAdapter, SpecialAdapter},
        request_profile::RequestClientProfile,
    },
    routing::candidate::{ProviderAccountPayload, ProviderExecutionMode},
    upstream::common::RequestPlan,
};

pub const PRESET_ID: &str = "grok-oauth";
pub const AUTH_MODE: &str = "grok_build_oauth";
pub const PROFILE: &str = "grok_build_oauth";

pub fn owns_base(base: &str) -> bool {
    url::Url::parse(base)
        .ok()
        .is_some_and(|url| url.host_str() == Some("cli-chat-proxy.grok.com"))
}
pub fn valid_base(base: &str) -> bool {
    base.trim_end_matches('/') == BUSINESS_BASE_URL
}
pub fn owns_payload(payload: &ProviderAccountPayload) -> bool {
    payload.auth_mode.as_deref() == Some(AUTH_MODE) || owns_base(&payload.base_url)
}
pub fn reserved_header(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    foreign_identity(&name)
        || matches!(
            name.as_str(),
            "authorization"
                | "proxy-authorization"
                | "cookie"
                | "host"
                | "api-key"
                | "x-api-key"
                | "x-goog-api-key"
                | "x-xai-token-auth"
                | "user-agent"
                | "content-type"
        )
}
pub fn foreign_identity(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    name.starts_with("x-stainless-")
        || matches!(
            name.as_str(),
            "originator"
                | "chatgpt-account-id"
                | "openai-organization"
                | "openai-project"
                | "x-requested-with"
                | "x-app"
                | "version"
                | "anthropic-version"
                | "anthropic-beta"
                | "anthropic-dangerous-direct-browser-access"
                | "x-ide-type"
                | "x-ide-name"
                | "x-ide-version"
                | "x-product"
                | "x-codebuddy-request"
        )
}
pub fn reserved_body_key(name: &str) -> bool {
    matches!(
        name,
        "access_token"
            | "grok_oauth"
            | "refresh_token"
            | "client_id"
            | "client_secret"
            | "scope"
            | "principal_type"
            | "principal_id"
            | "authorization"
            | "cookie"
            | "headers"
            | "base_url"
    )
}

pub fn validate(payload: &ProviderAccountPayload, adapter: &str) -> Result<(), GatewayError> {
    if !owns_payload(payload) {
        return Ok(());
    }
    if !cfg!(feature = "family-openai-compatible-official-api") {
        return Err(
            GatewayError::conflict("Grok Build transport was compiled out.")
                .with_code("gateway_provider_line_compiled_out"),
        );
    }
    if !valid_base(&payload.base_url)
        || payload.adapter != "openai_compatible"
        || adapter != "openai_compatible"
        || payload.auth_mode.as_deref() != Some(AUTH_MODE)
        || !matches!(
            payload.request_client_profile,
            None | Some(RequestClientProfile::Auto)
        )
        || payload.session_auth.is_some()
        || payload.keepalive.is_some()
        || payload.auth_header_name.is_some()
        || payload.auth_token.is_some()
        || !payload.discovered_protocols.is_empty()
        || payload.responses_path.as_deref() != Some("/responses")
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
        || payload
            .execution_mode
            .is_some_and(|m| m != ProviderExecutionMode::DirectHttp)
        || payload
            .endpoint_execution_modes
            .as_ref()
            .is_some_and(|modes| {
                modes
                    .values()
                    .any(|m| *m != ProviderExecutionMode::DirectHttp)
            })
        || payload.headers.keys().any(|h| reserved_header(h))
        || payload
            .extra_body
            .as_ref()
            .is_some_and(|body| body.keys().any(|k| reserved_body_key(k)))
        || payload.call_flow.is_some_and(|f| {
            !matches!(f.call_adapter, CallAdapter::Auto | CallAdapter::Openai)
                || !matches!(
                    f.special_adapter,
                    SpecialAdapter::Auto | SpecialAdapter::None
                )
                || f.fingerprint_adapter != FingerprintAdapter::Auto
        })
    {
        return Err(invalid());
    }
    Ok(())
}

pub fn validate_material(payload: &ProviderAccountPayload) -> Result<(), GatewayError> {
    if payload.api_key.is_empty()
        || payload.api_key.len() > 32 * 1024
        || !payload.api_key.bytes().all(|b| b.is_ascii_graphic())
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
    validate_material(payload)?;
    if plan.method != rquest::Method::POST
        || plan.url != format!("{BUSINESS_BASE_URL}/responses")
        || !plan.query.is_empty()
        || plan.response_kind != crate::protocol::canonical::EndpointKind::Responses
    {
        return Err(invalid());
    }
    let body = plan
        .body
        .as_ref()
        .and_then(serde_json::Value::as_object)
        .ok_or_else(invalid)?;
    if !body
        .get("input")
        .is_some_and(|v| v.is_string() || v.is_array())
        || !body
            .get("model")
            .is_some_and(|v| v.as_str().is_some_and(|s| !s.is_empty()))
        || !body
            .get("stream")
            .is_some_and(serde_json::Value::is_boolean)
        || body
            .keys()
            .any(|k| reserved_body_key(k) || matches!(k.as_str(), "messages" | "prompt"))
    {
        return Err(invalid());
    }
    Ok(())
}

pub(crate) fn invalid() -> GatewayError {
    GatewayError::bad_request("Invalid Grok Build OAuth product configuration.")
        .with_code("invalid_grok_build_oauth")
}
