use super::projection::get_or_rebuild_access_projection;
use super::projection_queries::{
    load_provider_accounts_with_any_credentials, load_provider_credential_map_for_rows,
};
use super::route_filter::{credential_payload_supports_route_row, route_rows_for_endpoint};
use super::*;

pub async fn preview_access_candidates(
    pool: &PgPool,
    redis_pool: &RedisPool,
    access_key_id: &str,
    requested_model: &str,
    endpoint_kind: EndpointKind,
    estimated_tokens: u64,
    explicit_session_key: Option<&str>,
) -> Result<Vec<GatewayAccessCandidatePreviewView>, GatewayError> {
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
    let credential_map = load_provider_credential_map_for_rows(pool, &provider_account_ids).await?;
    let accounts_with_credentials =
        load_provider_accounts_with_any_credentials(pool, &provider_account_ids).await?;
    let mut previews = Vec::with_capacity(rows.len());
    for row in rows {
        let balance = evaluate_access_key_balance(
            pool,
            redis_pool,
            &row.source_access_key_id,
            estimated_tokens,
        )
        .await?;
        if let Some(credentials) = credential_map.get(row.provider_account_id.as_str()) {
            for credential in credentials {
                if !credential_payload_supports_route_row(&credential.payload, &row) {
                    continue;
                }
                previews.push(GatewayAccessCandidatePreviewView {
                    requesting_access_key_id: row.requesting_access_key_id.clone(),
                    source_access_key_id: row.source_access_key_id.clone(),
                    platform_access_id: row.platform_access_id.clone(),
                    provider_account_id: row.provider_account_id.clone(),
                    provider_credential_id: Some(credential.id.clone()),
                    model_code: row.model_code.clone(),
                    endpoint_kind: row.endpoint_kind.clone(),
                    upstream_model: row.upstream_model.clone(),
                    platform_tier: row.platform_tier.clone(),
                    operator_weight: row.operator_weight,
                    routing_priority: row.routing_priority,
                    sticky_matched: sticky.as_ref().is_some_and(|value| {
                        value.platform_access_id == row.platform_access_id
                            && value.real_credential_ref.as_deref() == Some(credential.id.as_str())
                    }),
                    available_by_balance: balance.allowed,
                    balance_mode: balance.balance_mode.clone(),
                    remaining_tokens: balance.remaining_tokens,
                    remaining_messages: balance.remaining_messages,
                });
            }
        } else {
            if accounts_with_credentials.contains(row.provider_account_id.as_str()) {
                continue;
            }
            previews.push(GatewayAccessCandidatePreviewView {
                requesting_access_key_id: row.requesting_access_key_id.clone(),
                source_access_key_id: row.source_access_key_id.clone(),
                platform_access_id: row.platform_access_id.clone(),
                provider_account_id: row.provider_account_id.clone(),
                provider_credential_id: None,
                model_code: row.model_code.clone(),
                endpoint_kind: row.endpoint_kind.clone(),
                upstream_model: row.upstream_model.clone(),
                platform_tier: row.platform_tier.clone(),
                operator_weight: row.operator_weight,
                routing_priority: row.routing_priority,
                sticky_matched: sticky.as_ref().is_some_and(|value| {
                    value.platform_access_id == row.platform_access_id
                        && value.real_credential_ref.is_none()
                }),
                available_by_balance: balance.allowed,
                balance_mode: balance.balance_mode,
                remaining_tokens: balance.remaining_tokens,
                remaining_messages: balance.remaining_messages,
            });
        }
    }
    previews.sort_by(|left, right| {
        right
            .sticky_matched
            .cmp(&left.sticky_matched)
            .then(right.routing_priority.cmp(&left.routing_priority))
            .then(right.operator_weight.cmp(&left.operator_weight))
            .then(
                platform_tier_rank(&right.platform_tier)
                    .cmp(&platform_tier_rank(&left.platform_tier)),
            )
            .then(left.provider_account_id.cmp(&right.provider_account_id))
            .then(
                left.provider_credential_id
                    .as_deref()
                    .unwrap_or_default()
                    .cmp(right.provider_credential_id.as_deref().unwrap_or_default()),
            )
    });
    Ok(previews)
}

pub async fn preview_route_decision(
    pool: &PgPool,
    redis_pool: &RedisPool,
    access_key_id: &str,
    requested_model: &str,
    endpoint_kind: EndpointKind,
    estimated_tokens: u64,
    explicit_session_key: Option<&str>,
) -> Result<GatewayAccessRouteDecisionPreviewView, GatewayError> {
    let previews = preview_access_candidates(
        pool,
        redis_pool,
        access_key_id,
        requested_model,
        endpoint_kind,
        estimated_tokens,
        explicit_session_key,
    )
    .await?;
    let selected = previews
        .iter()
        .find(|candidate| candidate.sticky_matched && candidate.available_by_balance)
        .cloned()
        .or_else(|| {
            previews
                .iter()
                .find(|candidate| candidate.available_by_balance)
                .cloned()
        });
    Ok(GatewayAccessRouteDecisionPreviewView {
        requesting_access_key_id: access_key_id.to_string(),
        requested_model: requested_model.to_string(),
        endpoint_kind: endpoint_kind_name(endpoint_kind).to_string(),
        sticky_matched: selected
            .as_ref()
            .is_some_and(|candidate| candidate.sticky_matched),
        selected,
        candidates: previews,
    })
}
