//! Shared request identity/defaults for text APIs, never for browser/product transports.
mod body;
mod codex_identity;
pub(crate) mod context;
mod cpa;
mod cpa_full;
#[cfg(test)]
mod cpa_full_tests;
mod cpa_wire;
mod headers;
mod workbuddy;
mod workbuddy_codex;
mod workbuddy_transport;

use crate::protocol::canonical::{CanonicalRelayRequest, EndpointKind};
use crate::protocol::request_profile::RequestClientProfile;
use crate::routing::candidate::ProviderAccountPayload;

pub(crate) use body::apply_body;
pub(crate) use cpa::{identity as is_cpa_identity, transport as apply_cpa_transport_headers};
pub(crate) use cpa_full::{headers as finalize_cpa_full_headers, prepare as prepare_cpa_full};
pub(crate) use cpa_wire::{order as order_cpa_headers, plan as apply_cpa_plan};
pub(crate) use headers::apply_headers;
pub(crate) use workbuddy::is_identity as is_workbuddy_identity;
pub(crate) use workbuddy::order as order_workbuddy_headers;
pub(crate) use workbuddy_transport::apply as apply_workbuddy_transport;

pub(crate) fn applies(payload: &ProviderAccountPayload, req: &CanonicalRelayRequest) -> bool {
    // Basispoints canonicalizes to OpenAI for routing, not for product-specific wire identity.
    payload.adapter != crate::protocol::chatgpt::basispoints::ADAPTER
        && matches!(
            req.endpoint_kind,
            EndpointKind::Responses
                | EndpointKind::ChatCompletions
                | EndpointKind::Completions
                | EndpointKind::Messages
        )
        && matches!(
            payload.canonical_adapter(),
            "openai_compatible"
                | "anthropic_compatible"
                | "gemini_api_compatible"
                | "gemini_api_modular_compatible"
                | "cohere_compatible"
                | "bedrock_converse_compatible"
                | "dashscope_compatible"
                | "dashscope_multimodal_compatible"
                | "custom_http"
        )
}

fn selected(payload: &ProviderAccountPayload) -> RequestClientProfile {
    payload.request_client_profile.unwrap_or_default()
}

fn uses_claude(payload: &ProviderAccountPayload, req: &CanonicalRelayRequest, model: &str) -> bool {
    selected(payload).uses_claude(model, req.requested_model.as_deref())
}
