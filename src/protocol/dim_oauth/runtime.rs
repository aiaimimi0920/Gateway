//! DIM owns only these conversational wires; bearer authority cannot escape via adapters or paths.
use super::{business, Site};
use crate::{
    error::GatewayError,
    routing::candidate::{ProviderAccountPayload, ProviderExecutionMode},
    upstream::common::RequestPlan,
};

pub fn protocol_profile(family: &str) -> &'static str {
    match family {
        "openai_chat" => "dim_user_oauth_chat",
        "openai_responses" => "dim_user_oauth_responses",
        "anthropic_messages" => "dim_user_oauth_messages",
        "gemini_generate_content" => "dim_user_oauth_gemini",
        _ => unreachable!("DIM projection must produce one of its four declared wires"),
    }
}

pub fn validate(payload: &ProviderAccountPayload, adapter: &str) -> Result<(), GatewayError> {
    if !business::owns_payload(payload) {
        return Ok(());
    }
    let valid_base = [Site::China, Site::Global].into_iter().any(|site| {
        payload.base_url
            == match adapter {
                "gemini_api_compatible" => format!("{}/v1beta", site.issuer()),
                _ => business::base(site),
            }
    });
    if !cfg!(feature = "line-dim-user-oauth")
        || !valid_base
        || payload.auth_mode.as_deref() != Some(business::AUTH_MODE)
        || !matches!(
            adapter,
            "openai_compatible" | "anthropic_compatible" | "gemini_api_compatible"
        )
        || payload.auth_token.is_some()
        || payload.session_auth.is_some()
        || payload.keepalive.is_some()
        || !payload.headers.is_empty()
        || payload.extra_body.as_ref().is_some_and(|v| !v.is_empty())
        || !payload.discovered_protocols.is_empty()
        || payload.auth_header_name.is_some()
        || payload
            .request_client_profile
            .is_some_and(|p| p != crate::protocol::request_profile::RequestClientProfile::Auto)
        || payload
            .execution_mode
            .is_some_and(|m| m != ProviderExecutionMode::DirectHttp)
        || payload
            .endpoint_execution_modes
            .as_ref()
            .is_some_and(|v| !v.is_empty())
        || payload.call_flow.is_some_and(|f| {
            f.call_adapter != crate::protocol::call_flow::CallAdapter::Auto
                || !matches!(
                    f.special_adapter,
                    crate::protocol::call_flow::SpecialAdapter::Auto
                        | crate::protocol::call_flow::SpecialAdapter::None
                )
                || f.fingerprint_adapter != crate::protocol::call_flow::FingerprintAdapter::Auto
        })
        || !payload.api_key.is_empty() && !business::valid_token(&payload.api_key)
    {
        return Err(business::invalid());
    }
    Ok(())
}
pub fn validate_plan(
    payload: &ProviderAccountPayload,
    plan: &RequestPlan,
) -> Result<(), GatewayError> {
    if !business::owns_payload(payload) {
        return Ok(());
    }
    validate(payload, payload.canonical_adapter())?;
    let valid = match payload.canonical_adapter() {
        "openai_compatible" => ["/chat/completions", "/responses"]
            .iter()
            .any(|p| plan.url == format!("{}{p}", payload.base_url)),
        "anthropic_compatible" => plan.url == format!("{}/messages", payload.base_url),
        "gemini_api_compatible" => {
            let prefix = format!("{}/models/", payload.base_url);
            plan.url
                .strip_prefix(&prefix)
                .and_then(|v| {
                    v.strip_suffix(":streamGenerateContent")
                        .or_else(|| v.strip_suffix(":generateContent"))
                })
                .is_some_and(|model| {
                    !model.is_empty()
                        && model
                            .bytes()
                            .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
                })
        }
        _ => false,
    };
    if !valid
        || plan.method != rquest::Method::POST
        || plan.query.iter().any(|(k, v)| k != "alt" || v != "sse")
        || payload.api_key.is_empty()
    {
        return Err(business::invalid());
    }
    Ok(())
}
pub fn headers(payload: &ProviderAccountPayload) -> rquest::header::HeaderMap {
    let mut headers = rquest::header::HeaderMap::new();
    headers.insert(
        "content-type",
        rquest::header::HeaderValue::from_static("application/json"),
    );
    headers.insert(
        "user-agent",
        rquest::header::HeaderValue::from_static("NeuroGateway/0.1 dim-oauth"),
    );
    headers.insert(
        "x-title",
        rquest::header::HeaderValue::from_static("Neuro Gateway"),
    );
    if let Ok(value) = rquest::header::HeaderValue::from_str(&format!("Bearer {}", payload.api_key))
    {
        headers.insert("authorization", value);
    }
    if payload.canonical_adapter() == "anthropic_compatible" {
        headers.insert(
            "anthropic-version",
            rquest::header::HeaderValue::from_static("2023-06-01"),
        );
    }
    headers
}
