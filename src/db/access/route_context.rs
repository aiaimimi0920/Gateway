use super::projection::get_or_rebuild_access_projection;
use super::projection_queries::{
    load_provider_accounts_with_any_credentials, load_provider_candidate_meta_map,
    load_provider_credential_map_for_rows,
};
use super::route_filter::{credential_payload_supports_route_row, route_rows_for_endpoint};
use super::*;

pub async fn resolve_access_key_route_context(
    pool: &PgPool,
    redis_pool: &RedisPool,
    access_key_id: &str,
    requested_model: &str,
    endpoint_kind: EndpointKind,
    estimated_tokens: u64,
    explicit_session_key: Option<&str>,
) -> Result<ResolvedAccessKeyRouteContext, GatewayError> {
    let access_key = find_access_key_auth_by_id(pool, access_key_id)
        .await?
        .ok_or_else(|| GatewayError::not_found("access key 不存在"))?;
    validate_access_key_auth(&access_key)?;
    let projection = get_or_rebuild_access_projection(pool, redis_pool, &access_key).await?;
    let sticky = inspect_access_sticky_affinity(
        redis_pool,
        access_key_id,
        requested_model,
        explicit_session_key,
    )
    .await?;
    let rows = route_rows_for_endpoint(&projection, requested_model, endpoint_kind);
    let provider_account_ids = rows
        .iter()
        .map(|row| row.provider_account_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let provider_meta = load_provider_candidate_meta_map(pool, &provider_account_ids).await?;
    let credential_map = load_provider_credential_map_for_rows(pool, &provider_account_ids).await?;
    let accounts_with_credentials =
        load_provider_accounts_with_any_credentials(pool, &provider_account_ids).await?;

    let mut selected = None;
    let mut route_candidates = Vec::new();
    for row in rows {
        let balance = evaluate_access_key_balance(
            pool,
            redis_pool,
            &row.source_access_key_id,
            estimated_tokens,
        )
        .await?;
        if !balance.allowed {
            continue;
        }
        let Some(provider_row) = provider_meta.get(&row.provider_account_id) else {
            continue;
        };
        if provider_row
            .cooldown_until
            .is_some_and(|value| value > now_utc())
        {
            continue;
        }
        let sticky_matched = sticky
            .as_ref()
            .is_some_and(|value| value.platform_access_id == row.platform_access_id);
        let account_payload = get_provider_payload(pool, &row.provider_account_id)
            .await?
            .map(|view| view.payload);
        if let Some(credentials) = credential_map.get(row.provider_account_id.as_str()) {
            let Some(account_payload) = account_payload.clone() else {
                continue;
            };
            for credential in credentials {
                if !credential_payload_supports_route_row(&credential.payload, &row) {
                    continue;
                }
                let merged_payload = merge_provider_account_and_credential_payloads(
                    &account_payload,
                    &credential.payload,
                );
                let supported_protocol_families =
                    resolve_supported_wire_protocol_families_for_model(
                        Some(&merged_payload),
                        Some(requested_model),
                        row.upstream_model.as_deref(),
                        &provider_row.adapter,
                        &provider_row.protocol_family,
                    );
                let mut payload = crate::routing::candidate::deserialize_provider_payload(
                    &merged_payload,
                    Some(&provider_row.adapter),
                )
                .map_err(|error| {
                    GatewayError::server_error(format!(
                        "deserialize provider credential payload for {}: {error}",
                        credential.id
                    ))
                })?;
                if !payload.supports_search_endpoint(endpoint_kind) {
                    continue;
                }
                if payload.default_model.is_none() {
                    payload.default_model = Some(row.model_code.clone());
                }
                payload.credential_id = Some(credential.id.clone());
                let credential_sticky = sticky_matched
                    && sticky
                        .as_ref()
                        .and_then(|value| value.real_credential_ref.as_deref())
                        == Some(credential.id.as_str());
                if selected.is_none() && credential_sticky {
                    selected = Some(row.clone());
                }
                route_candidates.push((
                    credential_sticky,
                    row.clone(),
                    RouteCandidate {
                        provider_account_id: provider_row.id.clone(),
                        provider_credential_id: Some(credential.id.clone()),
                        label: format!("{} / {}", provider_row.label, credential.label),
                        payload,
                        protocol_family: canonicalize_protocol_family_name(
                            &provider_row.protocol_family,
                        ),
                        protocol_profile: provider_row.protocol_profile.clone(),
                        supported_protocol_families,
                        adapter: canonicalize_adapter_name(&provider_row.adapter),
                        model_alias: Some(requested_model.to_string()),
                        upstream_model: row.upstream_model.clone(),
                        resolved_execution_mode: match provider_row.execution_mode.as_str() {
                            "browser_backed" => ProviderExecutionMode::BrowserBacked,
                            _ => ProviderExecutionMode::DirectHttp,
                        },
                        priority: 0,
                        weight: 1,
                        failure_count: credential.failure_count.max(0) as u32,
                        cooldown_until: credential.cooldown_until.clone(),
                        routing_score: None,
                        routing_health_weight: None,
                        routing_capacity_weight: None,
                        routing_degraded: Some(credential.failure_count > 0),
                        routing_breaker_open: Some(false),
                        routing_degradation_reasons: if credential.failure_count > 0 {
                            vec!["provider_credential_failures".to_string()]
                        } else {
                            Vec::new()
                        },
                    },
                ));
            }
        } else {
            if accounts_with_credentials.contains(row.provider_account_id.as_str()) {
                continue;
            }
            let Some(payload_view) = get_provider_payload(pool, &row.provider_account_id).await?
            else {
                continue;
            };
            let supported_protocol_families = resolve_supported_wire_protocol_families_for_model(
                Some(&payload_view.payload),
                Some(requested_model),
                row.upstream_model.as_deref(),
                &provider_row.adapter,
                &provider_row.protocol_family,
            );
            let mut payload = crate::routing::candidate::deserialize_provider_payload(
                &payload_view.payload,
                Some(&provider_row.adapter),
            )
            .map_err(|error| {
                GatewayError::server_error(format!(
                    "deserialize provider payload for {}: {error}",
                    row.provider_account_id
                ))
            })?;
            if !payload.supports_search_endpoint(endpoint_kind) {
                continue;
            }
            if selected.is_none() && sticky_matched {
                selected = Some(row.clone());
            }
            if payload.default_model.is_none() {
                payload.default_model = Some(row.model_code.clone());
            }
            let upstream_model = row.upstream_model.clone();
            route_candidates.push((
                sticky_matched,
                row,
                RouteCandidate {
                    provider_account_id: provider_row.id.clone(),
                    provider_credential_id: None,
                    label: provider_row.label.clone(),
                    payload,
                    protocol_family: canonicalize_protocol_family_name(
                        &provider_row.protocol_family,
                    ),
                    protocol_profile: provider_row.protocol_profile.clone(),
                    supported_protocol_families,
                    adapter: canonicalize_adapter_name(&provider_row.adapter),
                    model_alias: Some(requested_model.to_string()),
                    upstream_model,
                    resolved_execution_mode: match provider_row.execution_mode.as_str() {
                        "browser_backed" => ProviderExecutionMode::BrowserBacked,
                        _ => ProviderExecutionMode::DirectHttp,
                    },
                    priority: 0,
                    weight: 1,
                    failure_count: provider_row.failure_count.max(0) as u32,
                    cooldown_until: provider_row.cooldown_until.map(format_timestamp),
                    routing_score: None,
                    routing_health_weight: None,
                    routing_capacity_weight: None,
                    routing_degraded: Some(provider_row.failure_count > 0),
                    routing_breaker_open: Some(false),
                    routing_degradation_reasons: if provider_row.failure_count > 0 {
                        vec!["provider_failures".to_string()]
                    } else {
                        Vec::new()
                    },
                },
            ));
        }
    }

    route_candidates.sort_by(|left, right| {
        right
            .0
            .cmp(&left.0)
            .then(right.1.routing_priority.cmp(&left.1.routing_priority))
            .then(right.1.operator_weight.cmp(&left.1.operator_weight))
            .then(
                platform_tier_rank(&right.1.platform_tier)
                    .cmp(&platform_tier_rank(&left.1.platform_tier)),
            )
            .then(left.1.provider_account_id.cmp(&right.1.provider_account_id))
    });
    if selected.is_none() {
        selected = route_candidates.first().map(|entry| entry.1.clone());
    }

    Ok(ResolvedAccessKeyRouteContext {
        requesting_access_key_id: access_key.id.clone(),
        key_kind: access_key.key_kind.clone(),
        model: requested_model.to_string(),
        sticky,
        selected,
        candidates: route_candidates
            .iter()
            .map(|entry| entry.1.clone())
            .collect(),
        route_candidates: route_candidates.into_iter().map(|entry| entry.2).collect(),
    })
}
