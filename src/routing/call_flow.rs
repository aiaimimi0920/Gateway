//! Pure ingress preparation. Resolve adapters while raw model policy is still available.
use crate::error::GatewayError;
use crate::protocol::{call_flow::prepare, canonical::EndpointKind, registry};
use crate::routing::candidate::{
    canonicalize_adapter_name, ProviderAccountPayload, RouteCandidate,
};
use serde::Deserialize;
use serde_json::Value;
use std::borrow::Cow;

pub(crate) struct PreparedRoutePayload {
    pub payload: ProviderAccountPayload,
    pub adapter: String,
    pub protocol_family: String,
    pub supported_protocol_families: Vec<String>,
}

pub(crate) fn prepare_owned(
    mut payload: ProviderAccountPayload,
) -> Result<ProviderAccountPayload, GatewayError> {
    if let Cow::Owned(prepared) = prepare(&payload)?.payload {
        payload = prepared;
    }
    Ok(payload)
}

/// The parsed policy never comes from headers/extra_body and is never forwarded upstream.
pub(crate) fn hydrate_cached_policy(
    payload: &mut ProviderAccountPayload,
    raw: Option<&Value>,
) -> Result<(), serde_json::Error> {
    #[derive(Default, Deserialize)]
    struct Policy {
        #[serde(default, alias = "callFlow")]
        call_flow: Option<crate::protocol::call_flow::ProviderCallFlow>,
        #[serde(default, alias = "requestClientProfile")]
        request_client_profile: Option<crate::protocol::request_profile::RequestClientProfile>,
    }
    let Some(raw) = raw.filter(|raw| raw.is_object()) else {
        return Ok(());
    };
    let policy = Policy::deserialize(raw)?;
    payload.call_flow = policy.call_flow;
    payload.request_client_profile = policy.request_client_profile;
    Ok(())
}

pub(crate) fn effective_family(payload: &ProviderAccountPayload, legacy: &str) -> String {
    // Fixed API formats own the default wire family, not the provider's stale DB column.
    if payload
        .call_flow
        .is_some_and(|flow| flow.call_adapter.adapter().is_some())
    {
        registry::canonicalize_wire_protocol_family_key(
            registry::default_protocol_family_for_adapter(&payload.adapter),
        )
    } else {
        registry::canonicalize_protocol_family_key(legacy)
    }
}

pub(crate) fn prepare_route_payload(
    payload: ProviderAccountPayload,
    raw: Option<&Value>,
    legacy_family: &str,
    model_alias: Option<&str>,
    upstream_model: Option<&str>,
) -> Result<Option<PreparedRoutePayload>, GatewayError> {
    let payload = prepare_owned(payload)?;
    let adapter = canonicalize_adapter_name(&payload.adapter);
    let protocol_family = effective_family(&payload, legacy_family);
    let mut supported_protocol_families =
        super::protocol_resolution::resolve_supported_wire_protocol_families_for_model(
            raw,
            model_alias,
            upstream_model,
            &adapter,
            &protocol_family,
        );
    if payload.call_flow.is_some_and(|flow| {
        flow.special_adapter == crate::protocol::call_flow::SpecialAdapter::FlowmusicWeb
    }) {
        supported_protocol_families
            .retain(|family| family == crate::protocol::registry::PRODUCER_MUSIC_FAMILY);
    }
    // Empty after applying explicit model blocks means deny, never "use all surfaces".
    if supported_protocol_families.is_empty() {
        return Ok(None);
    }
    Ok(Some(PreparedRoutePayload {
        payload,
        adapter,
        protocol_family,
        supported_protocol_families,
    }))
}

pub(crate) fn prepare_candidate(
    candidate: &mut RouteCandidate,
    endpoint: EndpointKind,
) -> Result<(), GatewayError> {
    let flow = prepare(&candidate.payload)?;
    if candidate.payload.call_flow.is_some_and(|flow| {
        flow.special_adapter == crate::protocol::call_flow::SpecialAdapter::FlowmusicWeb
    }) && endpoint != EndpointKind::MusicGenerations
    {
        return Err(GatewayError::bad_request(
            "FlowMusic Web supports only /v1/music/generations.",
        )
        .with_code("unsupported_flowmusic_endpoint"));
    }
    if candidate.payload.call_flow.is_none() {
        return Ok(());
    }
    let adapter = canonicalize_adapter_name(&flow.payload.adapter);
    if candidate.adapter != adapter
        || candidate
            .payload
            .call_flow
            .is_some_and(|flow| flow.call_adapter.adapter().is_some())
    {
        candidate.protocol_profile =
            registry::infer_protocol_profile(&adapter, None, Some(&flow.payload.base_url));
        candidate.protocol_family = effective_family(&flow.payload, &candidate.protocol_family);
    }
    if let Cow::Owned(payload) = flow.payload {
        candidate.payload = payload;
    }
    candidate.adapter = adapter;
    candidate.resolved_execution_mode = candidate.payload.resolve_execution_mode(endpoint);
    // Do not rebuild supported families here: the raw model allow/block policy is gone.
    Ok(())
}

pub(crate) fn validate_prepared_candidate(candidate: &RouteCandidate) -> Result<(), GatewayError> {
    let flow = prepare(&candidate.payload)?;
    if candidate.payload.call_flow.is_some() && flow.payload.adapter != candidate.adapter {
        return Err(GatewayError::bad_request(
            "Invalid call_flow: route candidate was not prepared before dispatch",
        ));
    }
    Ok(())
}
