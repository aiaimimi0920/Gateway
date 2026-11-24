//! Resolve before transport shortcuts; no I/O, credential mutation, or shared state.
use super::{CallAdapter, FingerprintAdapter, SpecialAdapter};
use crate::{error::GatewayError, routing::candidate::ProviderAccountPayload};
use std::borrow::Cow;

/// A prepared payload is request-local. Existing API encoders and multi-step executors
/// remain responsible for response decoding, streaming, cancellation and session cleanup.
pub struct PreparedCallFlow<'a> {
    pub payload: Cow<'a, ProviderAccountPayload>,
    pub call_adapter: &'a str,
    pub executor: &'a str,
    pub executor_owns_fingerprint: bool,
}

pub fn is_api_adapter(adapter: &str) -> bool {
    matches!(
        adapter,
        "openai_compatible"
            | "anthropic_compatible"
            | "gemini_api_compatible"
            | "gemini_api_modular_compatible"
            | "cohere_compatible"
            | "bedrock_converse_compatible"
            | "dashscope_compatible"
            | "dashscope_multimodal_compatible"
            | "custom_http"
            | "search_api_compatible"
            | "linkup_compatible"
    )
}

pub fn prepare(payload: &ProviderAccountPayload) -> Result<PreparedCallFlow<'_>, GatewayError> {
    crate::protocol::luma_mcp::boundary::reject_generic(payload)?;
    crate::protocol::agnes::validate(payload, payload.canonical_adapter())?;
    crate::protocol::luma_inference::validate(payload, payload.canonical_adapter())?;
    crate::protocol::sensenova::validate(payload, payload.canonical_adapter())?;
    crate::protocol::commandcode::validate(payload, payload.canonical_adapter())?;
    crate::protocol::sap_aicore::validate(payload, payload.canonical_adapter())?;
    crate::protocol::minimax::validate(payload, payload.canonical_adapter())?;
    crate::protocol::nous_portal::runtime::validate(payload, payload.canonical_adapter())?;
    crate::protocol::dim_oauth::runtime::validate(payload, payload.canonical_adapter())?;
    crate::protocol::cloudflare_workers_ai::validate(payload, payload.canonical_adapter())?;
    crate::protocol::luma_agents::validate(payload, payload.canonical_adapter())?;
    crate::protocol::vertex_express::validate(payload, payload.canonical_adapter())?;
    crate::protocol::vertex_oauth::business::validate(payload, payload.canonical_adapter())?;
    crate::protocol::kiro_api_key::validate(payload, payload.canonical_adapter())?;
    crate::protocol::kiro_oauth::runtime::validate(payload)?;
    crate::protocol::qwen::workspace_api::validate(payload, payload.canonical_adapter())?;
    crate::protocol::grok_oauth::business::validate(payload, payload.canonical_adapter())?;
    crate::protocol::perplexity_web::material::validate(payload, payload.canonical_adapter())?;
    let Some(flow) = payload.call_flow else {
        validate_bedrock_identity(payload, payload.canonical_adapter())?;
        return Ok(PreparedCallFlow {
            payload: Cow::Borrowed(payload),
            call_adapter: payload.canonical_adapter(),
            executor: if is_api_adapter(&payload.adapter) {
                "direct_http"
            } else {
                &payload.adapter
            },
            executor_owns_fingerprint: !is_api_adapter(&payload.adapter),
        });
    };
    let api = is_api_adapter(&payload.adapter);
    let antigravity = payload.adapter == crate::protocol::antigravity::ADAPTER;
    if (flow.fingerprint_adapter == FingerprintAdapter::Antigravity && !antigravity)
        || (antigravity
            && !matches!(
                flow.fingerprint_adapter,
                FingerprintAdapter::Auto | FingerprintAdapter::Antigravity
            ))
    {
        return Err(invalid(
            "Antigravity requires its dedicated executor-owned fingerprint",
        ));
    }
    if flow.special_adapter == SpecialAdapter::None && !api {
        return Err(invalid(
            "special_adapter=none cannot disable a preset-owned product executor",
        ));
    }
    if let Some(expected) = flow.special_adapter.adapter() {
        if payload.adapter != expected {
            return Err(invalid(
                "special_adapter does not match the configured preset/transport",
            ));
        }
    }
    if !api && flow.call_adapter != CallAdapter::Auto {
        return Err(invalid(
            "this specialized executor currently requires call_adapter=auto",
        ));
    }
    if api && flow.fingerprint_adapter == FingerprintAdapter::Session {
        return Err(invalid(
            "fingerprint_adapter=session requires a specialized executor",
        ));
    }
    if !api
        && !matches!(
            flow.fingerprint_adapter,
            FingerprintAdapter::Auto
                | FingerprintAdapter::Session
                | FingerprintAdapter::Antigravity
        )
    {
        return Err(invalid(
            "this specialized executor owns its fingerprint; API profiles are not supported",
        ));
    }
    let adapter = if api {
        flow.call_adapter.adapter().unwrap_or(&payload.adapter)
    } else {
        &payload.adapter
    };
    validate_bedrock_identity(payload, adapter)?;
    crate::protocol::cloudflare_workers_ai::validate(payload, adapter)?;
    crate::protocol::qwen::workspace_api::validate(payload, adapter)?;
    crate::protocol::grok_oauth::business::validate(payload, adapter)?;
    let profile = if api {
        flow.fingerprint_adapter.profile()
    } else {
        None
    };
    let mut effective = Cow::Borrowed(payload);
    if adapter != payload.adapter || profile != payload.request_client_profile {
        let updated = effective.to_mut();
        updated.adapter = adapter.to_owned();
        updated.request_client_profile = profile;
    }
    crate::protocol::vertex_express::validate(effective.as_ref(), adapter)?;
    crate::protocol::vertex_oauth::business::validate(effective.as_ref(), adapter)?;
    crate::protocol::nous_portal::runtime::validate(effective.as_ref(), adapter)?;
    tracing::trace!(call_adapter = adapter, executor = %payload.adapter,
        fingerprint = ?flow.fingerprint_adapter, "call flow prepared");
    let call_adapter = if adapter == payload.adapter {
        payload.canonical_adapter()
    } else {
        adapter
    };
    Ok(PreparedCallFlow {
        payload: effective,
        call_adapter,
        executor: if api { "direct_http" } else { &payload.adapter },
        executor_owns_fingerprint: !api,
    })
}

fn invalid(message: &str) -> GatewayError {
    GatewayError::bad_request(format!("Invalid call_flow: {message}"))
}

fn validate_bedrock_identity(
    payload: &ProviderAccountPayload,
    adapter: &str,
) -> Result<(), GatewayError> {
    // Run before any native shortcut, worker or alternate transport can consume the material.
    if crate::protocol::bedrock_converse::explicit_auth_mode(payload.auth_mode.as_deref())
        && adapter != "bedrock_converse_compatible"
    {
        return Err(invalid(
            "explicit Bedrock authentication cannot use another protocol or executor",
        ));
    }
    Ok(())
}
