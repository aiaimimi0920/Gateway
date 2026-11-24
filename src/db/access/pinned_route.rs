//! Hosted authorization for an existing receipt, without balance admission or fallback.
#![allow(dead_code)] // Consumed by the internal SAP lifecycle before public dispatch is wired.

use super::{
    projection::read_current_access_projection,
    projection_queries::load_provider_candidate_meta_map,
    route_filter::{credential_payload_supports_route_row, route_rows_for_endpoint},
    *,
};
use crate::{
    auth::session::AuthenticatedSession,
    routing::{
        call_flow::prepare_route_payload,
        candidate::{
            deserialize_provider_payload, ProviderAccountPayload, ProviderExecutionMode,
            RouteCandidate,
        },
    },
};

pub(crate) struct PinnedRoute<'a> {
    pub model: &'a str,
    pub source_key: &'a str,
    pub platform_access: &'a str,
    pub provider: &'a str,
    pub credential: &'a str,
    pub account_group: Option<&'a str>,
}

pub(crate) async fn read_current(
    pool: &PgPool,
    session: &AuthenticatedSession,
    routes: &crate::routing::config::RouteConfigStore,
    pin: PinnedRoute<'_>,
) -> Result<Option<(ProviderAccountPayload, String)>, GatewayError> {
    Ok(read_candidate(pool, session, routes, pin)
        .await?
        .map(|(candidate, model)| (candidate.payload, model)))
}

pub(crate) async fn read_candidate(
    pool: &PgPool,
    session: &AuthenticatedSession,
    routes: &crate::routing::config::RouteConfigStore,
    pin: PinnedRoute<'_>,
) -> Result<Option<(RouteCandidate, String)>, GatewayError> {
    let Some(key_id) = session.access_key_id.as_deref() else {
        return Ok(None);
    };
    let Some(key) = find_access_key_auth_by_id(pool, key_id).await? else {
        return Ok(None);
    };
    if key.resolved_project_id != session.project_id
        || key.resolved_tenant_id != session.tenant_id
        || validate_access_key_auth(&key).is_err()
        || !scope_list_from_metadata(key.metadata.as_ref())
            .iter()
            .any(|scope| scope == "relay" || scope == "responses")
    {
        return Ok(None);
    }
    let snapshot = routes.snapshot();
    let Ok(mut groups) = snapshot.account_group_constraint(pin.account_group) else {
        return Ok(None);
    };
    if let Some(ids) = crate::access_key_groups::group_ids(key.metadata.as_ref())? {
        groups.intersect(&snapshot.access_key_group_constraint(&ids));
    }
    if !groups.allows_account_id(pin.credential) {
        return Ok(None);
    }

    // A cached projection can outlive an aggregate member's expiry without a revision change.
    let projection = read_current_access_projection(pool, &key).await?;
    let mut rows = route_rows_for_endpoint(&projection, pin.model, EndpointKind::Responses)
        .into_iter()
        .filter(|row| {
            row.requesting_access_key_id == key_id
                && row.source_access_key_id == pin.source_key
                && row.platform_access_id == pin.platform_access
                && row.provider_account_id == pin.provider
        });
    let Some(row) = rows.next() else {
        return Ok(None);
    };
    let Some(credential) = crate::db::get_provider_credential(pool, pin.credential).await? else {
        return Ok(None);
    };
    // Several bundles may grant the same row; they are not alternative routes.
    if rows.any(|other| other.upstream_model != row.upstream_model)
        || credential.provider_account_id != pin.provider
        || credential.archived_at.is_some()
        || credential.status != "active"
        || !credential_payload_supports_route_row(&credential.payload, &row)
    {
        return Ok(None);
    }
    let Some(provider) = load_provider_candidate_meta_map(pool, &[pin.provider.to_owned()])
        .await?
        .remove(pin.provider)
    else {
        return Ok(None);
    };
    let Some(account) = get_provider_payload(pool, pin.provider).await? else {
        return Ok(None);
    };
    if account.status != "active" {
        return Ok(None);
    }
    let merged =
        merge_provider_account_and_credential_payloads(&account.payload, &credential.payload);
    let mut payload = deserialize_provider_payload(&merged, Some(&provider.adapter))
        .map_err(|_| GatewayError::service_unavailable("Pinned provider payload is invalid."))?;
    payload.adapter = canonicalize_adapter_name(&provider.adapter);
    let Some(mut prepared) = prepare_route_payload(
        payload,
        Some(&merged),
        &provider.protocol_family,
        Some(pin.model),
        row.upstream_model.as_deref(),
    )?
    else {
        return Ok(None);
    };
    prepared.payload.credential_id = Some(credential.id.clone());
    if routes.snapshot().revision() != snapshot.revision() {
        return Ok(None);
    }
    let model = row.upstream_model.unwrap_or(row.model_code);
    let candidate = RouteCandidate {
        provider_account_id: provider.id,
        provider_credential_id: Some(credential.id),
        label: format!("{} / {}", provider.label, credential.label),
        payload: prepared.payload,
        protocol_family: prepared.protocol_family,
        protocol_profile: prepared
            .protocol_profile
            .unwrap_or(provider.protocol_profile),
        supported_protocol_families: prepared.supported_protocol_families,
        adapter: prepared.adapter,
        model_alias: Some(pin.model.to_owned()),
        upstream_model: Some(model.clone()),
        resolved_execution_mode: match provider.execution_mode.as_str() {
            "browser_backed" => ProviderExecutionMode::BrowserBacked,
            _ => ProviderExecutionMode::DirectHttp,
        },
        priority: 0,
        weight: 1,
        failure_count: credential.failure_count.max(0) as u32,
        cooldown_until: credential.cooldown_until,
        routing_score: None,
        routing_health_weight: None,
        routing_capacity_weight: None,
        routing_degraded: None,
        routing_breaker_open: None,
        routing_degradation_reasons: Vec::new(),
    };
    Ok(Some((candidate, model)))
}
