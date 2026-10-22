//! Opt-in Claude identity shaping. Protocol, provider authentication and OAuth stay independent.
use super::*;
use crate::{error::GatewayError, upstream::common::RequestPlan};
use rquest::header::HeaderMap;

pub(crate) fn prepare(
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    plan: &mut RequestPlan,
) -> Result<(), GatewayError> {
    if selected(payload) != RequestClientProfile::CpaClaudeCode {
        return Ok(());
    }
    if payload.canonical_adapter() != "anthropic_compatible" || !applies(payload, req) {
        return Err(error(
            "CPA Claude Code identity requires the Anthropic Messages protocol",
        ));
    }
    let context = context::current()
        .ok_or_else(|| error("CPA Claude Code identity requires authenticated dispatch context"))?;
    let body = plan
        .body
        .as_mut()
        .ok_or_else(|| error("CPA Claude Code identity requires a JSON body"))?;
    // The generic Anthropic packer adds a top-level cache default. CPA owns its own
    // placement; retain an explicitly supplied marker, but not this Gateway default.
    if req.raw_body.get("cache_control").is_none()
        && !payload
            .extra_body
            .as_ref()
            .is_some_and(|extra| extra.contains_key("cache_control"))
    {
        if let Some(object) = body.as_object_mut() {
            object.remove("cache_control");
        }
    }
    if req.endpoint_kind == EndpointKind::Messages {
        if let (Some(original), Some(object)) = (req.raw_body.as_object(), body.as_object_mut()) {
            let mut ordered = serde_json::Map::new();
            for key in original.keys() {
                if let Some(value) = object.remove(key) {
                    ordered.insert(key.clone(), value);
                }
            }
            ordered.extend(std::mem::take(object));
            *object = ordered;
        }
    }
    if url::Url::parse(&payload.base_url)
        .ok()
        .and_then(|u| u.host_str().map(str::to_owned))
        .as_deref()
        == Some("api.anthropic.com")
    {
        return Err(error(
            "CPA Claude Code full identity currently requires a custom provider origin",
        ));
    }
    let format = match req.endpoint_kind {
        EndpointKind::Messages => "claude",
        EndpointKind::Responses => "openai-response",
        _ => "openai",
    };
    gateway_cpa_claude_profile::prepare_mode(
        body,
        &req.raw_body,
        &payload.api_key,
        context.session_hint.as_deref(),
        &context.caller_scope,
        format,
        context.native_claude,
    )
    .map_err(error)?;
    if req.endpoint_kind == EndpointKind::Responses {
        // Match the reference translator's base-object insertion order without
        // changing protocol conversion values or dropping operator extensions.
        if let Some(object) = body.as_object_mut() {
            let mut ordered = serde_json::Map::new();
            for key in [
                "model",
                "max_tokens",
                "messages",
                "metadata",
                "thinking",
                "output_config",
                "stream",
                "speed",
                "system",
                "tools",
                "tool_choice",
            ] {
                if let Some(value) = object.remove(key) {
                    ordered.insert(key.into(), value);
                }
            }
            ordered.extend(std::mem::take(object));
            *object = ordered;
        }
    }
    Ok(())
}

pub(crate) fn headers(
    payload: &ProviderAccountPayload,
    plan: &RequestPlan,
    headers: &mut HeaderMap,
) -> Result<(), GatewayError> {
    if selected(payload) != RequestClientProfile::CpaClaudeCode {
        return Ok(());
    }
    let body = plan
        .body
        .as_ref()
        .ok_or_else(|| error("missing CPA Claude Code body"))?;
    let session = gateway_cpa_claude_profile::session_header(body)
        .ok_or_else(|| error("missing CPA Claude Code session"))?;
    let context = context::current().ok_or_else(|| error("missing CPA dispatch context"))?;
    if context.native_claude {
        // Native SDK fields have platform defaults, unlike CPA's synthetic MacOS identity.
        let os = if cfg!(windows) {
            "Windows"
        } else if cfg!(target_os = "macos") {
            "MacOS"
        } else {
            "Linux"
        };
        let arch = if cfg!(target_arch = "aarch64") {
            "arm64"
        } else {
            "x64"
        };
        for (name, value) in [
            ("x-stainless-os", context.native_os.as_deref().unwrap_or(os)),
            (
                "x-stainless-arch",
                context.native_arch.as_deref().unwrap_or(arch),
            ),
        ] {
            // Provider configuration and custom authentication keep their existing authority.
            if !payload.headers.keys().any(|k| k.eq_ignore_ascii_case(name))
                && !payload
                    .auth_header_name
                    .as_deref()
                    .is_some_and(|k| k.eq_ignore_ascii_case(name))
            {
                headers.insert(
                    name,
                    value
                        .parse()
                        .map_err(|_| error("invalid native CPA platform header"))?,
                );
            }
        }
    }
    for (key, value) in [
        ("x-claude-code-session-id", session),
        (
            "anthropic-beta",
            gateway_cpa_claude_profile::select_beta_headers(
                body,
                &context.requested_betas,
                context.native_claude,
            ),
        ),
    ] {
        headers.insert(
            key,
            value
                .parse()
                .map_err(|_| error("invalid CPA Claude Code header"))?,
        );
    }
    Ok(())
}

fn error(message: &str) -> GatewayError {
    GatewayError::bad_request(message).with_code("cpa_identity_configuration")
}
