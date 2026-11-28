//! Materialize a concrete `RouteCandidate` and runtime OAuth overrides from a `CompiledProvider`.

use super::*;

/// Apply OAuth token override if a refreshed access_token is available.
pub(in crate::routing::config) fn apply_token_override(
    cred: &CompiledCredential,
) -> ProviderAccountPayload {
    let mut payload = cred.payload.clone();
    if let Some(ref config) = cred.refresh_config {
        if let Some((access, context, generation)) = config.lobster_dispatch_snapshot() {
            payload.api_key = access;
            crate::protocol::lobsterai::metadata::update(&mut payload, &context, generation);
            return payload;
        }

        let (override_key, expiry, target) = config.runtime_dispatch_override();
        if let Some(override_key) = override_key {
            payload.api_key = override_key;
        }
        if let Some(expiry) = expiry {
            payload.expires_at = Some(expiry.clone());
            if let Some(session_auth) = payload.session_auth.as_mut() {
                session_auth.expires_at = Some(expiry);
            }
        }
        if let Some(target) = target {
            payload.base_url = target.into();
        }
    }
    payload
}

/// Convert a `CompiledProvider` to a `RouteCandidate`.
///
/// `model` is the resolved model name (after alias resolution) to send
/// upstream; `alias` is the original alias if one was used.
///
/// When the provider has a credential pool, the candidate's `payload` is set
/// to a round-robin-selected credential, but `provider_account_id` always
/// references the provider — one AIMD controller for the whole pool.
pub(super) fn provider_to_candidate(
    p: &CompiledProvider,
    model: Option<&str>,
    alias: Option<&str>,
    priority: i32,
    allowed_account_ids: Option<&HashSet<String>>,
    admission: Admission<'_>,
    rotate: bool,
) -> Option<RouteCandidate> {
    // A group-scoped default account must not inherit compatibility fallback
    // for models outside its declared catalogue. Aliases are already resolved.
    if allowed_account_ids.is_some()
        && p.uses_provider_default_account()
        && !p.supported_models.is_empty()
        && model.is_some_and(|model| !model_mapping::provider_supports_model(p, model))
    {
        return None;
    }
    let (upstream_model, mut selected_payload) =
        if let Some(model) = model.filter(|m| p.model_map_targets.contains_key(*m)) {
            let (mapped, payload) = model_mapping::select_mapped_payload_filtered(
                p,
                model,
                allowed_account_ids,
                admission,
                rotate,
            )?;
            (Some(mapped), payload)
        } else {
            let translated = model.and_then(|m| p.model_map.get(m).map(String::as_str));
            (
                model.map(|m| translated.unwrap_or(m).to_string()),
                select_credential_filtered(
                    p,
                    model,
                    translated,
                    allowed_account_ids,
                    admission,
                    rotate,
                )?,
            )
        };
    let dim_family = if crate::protocol::dim_oauth::business::owns_payload(&selected_payload) {
        let catalog = p
            .credential_pool
            .iter()
            .find(|c| Some(c.id.as_str()) == selected_payload.credential_id.as_deref())?
            .dim_catalog
            .as_ref()?;
        let (effective, family) = catalog
            .project(&selected_payload, upstream_model.as_deref()?)
            .ok()?;
        selected_payload = effective;
        Some(family)
    } else {
        None
    };
    let resolved_execution_mode = selected_payload
        .resolve_execution_mode(crate::protocol::canonical::EndpointKind::ChatCompletions);
    let discovery = p
        .credential_pool
        .iter()
        .find(|c| Some(c.id.as_str()) == selected_payload.credential_id.as_deref())
        .and_then(|c| c.discovery.as_ref());
    let family =
        dim_family.unwrap_or_else(|| discovery.map(|d| d.family()).unwrap_or(&p.protocol_family));

    Some(RouteCandidate {
        max_concurrent_requests: selected_payload
            .credential_id
            .as_deref()
            .and_then(|id| {
                p.credential_pool
                    .iter()
                    .find(|credential| credential.id == id)
            })
            .and_then(|credential| credential.max_concurrent_requests)
            .or_else(|| {
                p.uses_provider_default_account()
                    .then_some(p.max_concurrent_requests)
                    .flatten()
            }),
        provider_account_id: p.id.clone(),
        provider_credential_id: None,
        label: p.label.clone(),
        adapter: selected_payload.adapter.clone(),
        protocol_family: family.to_owned(),
        protocol_profile: dim_family
            .map(crate::protocol::dim_oauth::runtime::protocol_profile)
            .map(str::to_owned)
            .unwrap_or_else(|| {
                super::super::probe_identity::effective_protocol_profile(p, discovery)
            }),
        supported_protocol_families: if dim_family.is_some() {
            vec![family.to_owned()]
        } else if selected_payload.call_flow.is_some_and(|flow| {
            flow.special_adapter == crate::protocol::call_flow::SpecialAdapter::FlowmusicWeb
        }) {
            vec![crate::protocol::registry::PRODUCER_MUSIC_FAMILY.to_string()]
        } else if discovery.is_some() {
            selected_payload
                .discovered_protocols
                .iter()
                .map(|c| c.protocol.family().to_owned())
                .collect()
        } else {
            crate::routing::protocol_resolution::payload_supported_wire_protocol_families(
                &selected_payload,
                family,
                upstream_model.as_deref(),
            )
        },
        payload: selected_payload,
        model_alias: alias.map(|s| s.to_string()),
        upstream_model,
        resolved_execution_mode,
        priority,
        weight: 100,
        failure_count: 0,
        cooldown_until: None,
        routing_score: None,
        routing_health_weight: None,
        routing_capacity_weight: None,
        routing_degraded: None,
        routing_breaker_open: None,
        routing_degradation_reasons: Vec::new(),
    })
}
