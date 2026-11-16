//! Compiler-owned directory projection; never merge this metadata into inference JSON.
use super::*;
use crate::{provider_discovery::model_catalog::ModelCatalog, routing::candidate::RouteCandidate};
const KEY: &str = "gateway_commandcode_catalog";

/// Restore a validated raw directory before product admission at DB/cache ingress.
pub(crate) fn hydrate(
    payload: &mut ProviderAccountPayload,
    raw: Option<&serde_json::Value>,
) -> Result<(), GatewayError> {
    let Some(raw) = raw else { return Ok(()) };
    let typed = ["auth_mode", "authMode"]
        .iter()
        .any(|key| raw.get(key).and_then(serde_json::Value::as_str) == Some(AUTH_MODE));
    if !owns_payload(payload) && !typed {
        return Ok(());
    }
    #[derive(serde::Deserialize)]
    struct Input {
        #[serde(default, alias = "authMode")]
        auth_mode: Option<String>,
        #[serde(default, alias = "modelCatalog")]
        model_catalog: Option<ModelCatalog>,
    }
    let input = <Input as serde::Deserialize>::deserialize(raw).map_err(|_| invalid())?;
    if payload
        .auth_mode
        .as_deref()
        .is_some_and(|mode| mode != AUTH_MODE)
        || input
            .auth_mode
            .as_deref()
            .is_some_and(|mode| mode != AUTH_MODE)
    {
        return Err(invalid());
    }
    // Raw cache reconstruction has no preset owner to restore this identity.
    payload.auth_mode = Some(AUTH_MODE.into());
    let Some(catalog) = input.model_catalog else {
        return Ok(());
    };
    catalog
        .validate(Some(PRESET), &payload.base_url)
        .map_err(|_| invalid())?;
    // Never erase caller body overrides to make an otherwise invalid account pass.
    if ["extra_body", "extraBody"].iter().any(|key| {
        raw.get(key).is_some_and(|value| {
            !value.is_null() && value.as_object().is_none_or(|object| !object.is_empty())
        })
    }) {
        return Err(invalid());
    }
    read(payload)?;
    configure(payload, &catalog);
    Ok(())
}

/// A local compiled account derives its capabilities from model declarations, not adapter defaults.
pub(crate) fn families(
    payload: &ProviderAccountPayload,
    model: Option<&str>,
) -> Option<Vec<String>> {
    if !owns_payload(payload) {
        return None;
    }
    let catalog = read(payload).ok()??;
    let routes = catalog.supported_endpoints.get(model?)?;
    Some(
        routes
            .iter()
            .filter_map(|route| match route.as_str() {
                "/chat/completions" => Some(crate::protocol::registry::OPENAI_CHAT_FAMILY.into()),
                "/responses" => Some(crate::protocol::registry::OPENAI_RESPONSES_FAMILY.into()),
                "/messages" => Some(crate::protocol::registry::ANTHROPIC_MESSAGES_FAMILY.into()),
                "/systemone" => Some(crate::protocol::registry::SYSTEMONE_FAMILY.into()),
                _ => None,
            })
            .collect(),
    )
}

pub(crate) fn configure(payload: &mut ProviderAccountPayload, catalog: &ModelCatalog) {
    if owns_payload(payload) && !catalog.supported_endpoints.is_empty() {
        payload.extra_body = Some(
            [(
                KEY.into(),
                serde_json::to_value(catalog).expect("catalog serializes"),
            )]
            .into(),
        );
    }
}

pub(super) fn read(payload: &ProviderAccountPayload) -> Result<Option<ModelCatalog>, GatewayError> {
    let Some(map) = payload.extra_body.as_ref().filter(|map| !map.is_empty()) else {
        return Ok(None);
    };
    if map.len() != 1 {
        return Err(invalid());
    }
    let catalog: ModelCatalog =
        serde_json::from_value(map.get(KEY).ok_or_else(invalid)?.clone()).map_err(|_| invalid())?;
    catalog
        .validate(Some(PRESET), BASE)
        .map_err(|_| invalid())?;
    Ok(Some(catalog))
}

pub(super) fn supports_responses(
    payload: &ProviderAccountPayload,
    model: &str,
) -> Result<bool, GatewayError> {
    supports(payload, model, "/responses")
}

pub(super) fn supports(
    payload: &ProviderAccountPayload,
    model: &str,
    route: &str,
) -> Result<bool, GatewayError> {
    let Some(catalog) = read(payload)? else {
        return Ok(route == "/chat/completions");
    };
    // Refreshing the directory must not disable manually configured Chat models.
    // Explicit declarations remain authoritative and never gain a Chat fallback.
    Ok(catalog
        .supported_endpoints
        .get(model)
        .map_or(route == "/chat/completions", |routes| {
            routes.iter().any(|value| value == route)
        }))
}

/// Pick once before a probe sends any inference; no endpoint retries to discover capability.
pub(super) fn probe_endpoint(
    payload: &ProviderAccountPayload,
    model: &str,
) -> Result<EndpointKind, GatewayError> {
    for (path, endpoint) in [
        ("/chat/completions", EndpointKind::ChatCompletions),
        ("/responses", EndpointKind::Responses),
        ("/messages", EndpointKind::Messages),
        ("/systemone", EndpointKind::SystemOne),
    ] {
        if supports(payload, model, path)? {
            return Ok(endpoint);
        }
    }
    Err(invalid())
}

/// Select on the request-local candidate, never mutate its saved account or declare generation verified.
pub(crate) fn select(
    candidate: &mut RouteCandidate,
    req: &crate::protocol::canonical::CanonicalRelayRequest,
    forced: Option<&str>,
) -> Result<bool, GatewayError> {
    let (path, family) = match req.endpoint_kind {
        EndpointKind::Responses => (
            "/responses",
            crate::protocol::registry::OPENAI_RESPONSES_FAMILY,
        ),
        EndpointKind::Messages => (
            "/messages",
            crate::protocol::registry::ANTHROPIC_MESSAGES_FAMILY,
        ),
        EndpointKind::SystemOne => ("/systemone", crate::protocol::registry::SYSTEMONE_FAMILY),
        _ => return Ok(false),
    };
    let native = forced.is_none_or(|value| value == family)
        && (candidate.supported_protocol_families.is_empty()
            || candidate.supported_protocol_families.iter().any(|allowed| {
                crate::protocol::registry::canonicalize_wire_protocol_family_key(allowed) == family
            }))
        && supports(
            &candidate.payload,
            candidate.upstream_model.as_deref().unwrap_or_default(),
            path,
        )?;
    if native {
        candidate.payload.chat_completions_path = None;
        candidate.payload.responses_path = (path == "/responses").then(|| path.into());
        candidate.payload.messages_path = (path == "/messages").then(|| path.into());
        candidate.protocol_family = family.into();
        candidate.priority = candidate.priority.saturating_add(
            crate::routing::protocol_resolution::SAME_PROTOCOL_FAMILY_PRIORITY_BONUS,
        );
    }
    Ok(native)
}
