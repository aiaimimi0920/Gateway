use super::*;

/// Resolve the database route candidates for a request, refusing an empty result.
///
/// Use this when the database is the only routing source available: an empty
/// candidate set is then a dead end and deserves an error naming the reason.
/// A deployment that also carries a route document should call
/// [`resolve_route_candidates_allowing_empty`] instead so the document can still
/// answer the request.
pub async fn resolve_route_candidates(
    pool: &PgPool,
    project_id: &str,
    requested_model: Option<&str>,
    endpoint_kind: EndpointKind,
) -> Result<ResolvedProjectRouteContext, GatewayError> {
    let resolved =
        resolve_route_candidates_allowing_empty(pool, project_id, requested_model, endpoint_kind)
            .await?;
    if resolved.candidates.is_empty() {
        return Err(GatewayError::conflict(
            "当前网关没有可用的 provider account",
        ));
    }
    Ok(resolved)
}

/// Resolve the database route candidates for a request, tolerating an empty result.
///
/// A standalone deployment keeps its providers in the route document rather than
/// in `gateway_provider_accounts`, so those tables are legitimately empty while
/// the gateway is fully routable. Returning an empty candidate list lets the
/// caller fall back to the route document; the route policy is still reported so
/// the request stays attributed to it.
///
/// A route policy that restricts models is a different matter: it is an operator
/// decision about what this project may call, so a model it forbids still fails
/// here rather than escaping to the route document.
pub async fn resolve_route_candidates_allowing_empty(
    pool: &PgPool,
    project_id: &str,
    requested_model: Option<&str>,
    endpoint_kind: EndpointKind,
) -> Result<ResolvedProjectRouteContext, GatewayError> {
    let project = get_active_project(pool, project_id).await?;
    let route_policy = get_active_route_policy_for_requests(
        pool,
        &project.id,
        project.default_route_policy_id.as_deref(),
    )
    .await?;
    let alias_rows = fetch_alias_rows(pool, &project.id, requested_model).await?;
    let provider_rows = fetch_active_provider_rows(pool).await?;
    let eligible_provider_rows = provider_rows
        .into_iter()
        .filter(|row| provider_allowed_by_route_policy(row, Some(&route_policy.config)))
        .collect::<Vec<_>>();
    let provider_credentials = list_active_provider_credentials_for_accounts(
        pool,
        &eligible_provider_rows
            .iter()
            .map(|row| row.id.clone())
            .collect::<Vec<_>>(),
    )
    .await?;
    let provider_map = eligible_provider_rows
        .iter()
        .map(|row| (row.id.as_str(), row))
        .collect::<HashMap<_, _>>();
    let mut credential_map = HashMap::<String, Vec<super::GatewayProviderCredentialView>>::new();
    for credential in provider_credentials {
        credential_map
            .entry(credential.provider_account_id.clone())
            .or_default()
            .push(credential);
    }

    let mut candidates = Vec::new();
    for alias_row in alias_rows {
        let Some(provider_row) = provider_map.get(alias_row.provider_account_id.as_str()) else {
            continue;
        };
        if !provider_allowed_by_route_policy(provider_row, Some(&route_policy.config)) {
            continue;
        }
        if !route_policy_allows_models(
            Some(&route_policy.config),
            [
                Some(alias_row.alias.as_str()),
                alias_row.upstream_model.as_deref(),
            ],
        ) {
            continue;
        }
        candidates.extend(
            build_route_candidates_for_provider(
                provider_row,
                credential_map.get(provider_row.id.as_str()),
                endpoint_kind,
                Some(alias_row.alias.clone()),
                alias_row.upstream_model.clone(),
                -alias_row.priority,
                alias_row.weight.max(1),
            )
            .await?,
        );
    }

    if candidates.is_empty() {
        for provider_row in eligible_provider_rows {
            let provider_candidates = build_route_candidates_for_provider(
                &provider_row,
                credential_map.get(provider_row.id.as_str()),
                endpoint_kind,
                requested_model.map(str::to_string),
                None,
                -100,
                1,
            )
            .await?;
            for candidate in provider_candidates {
                let default_model = candidate
                    .payload
                    .default_model
                    .clone()
                    .or_else(|| requested_model.map(str::to_string));
                if !route_policy_allows_models(
                    Some(&route_policy.config),
                    [requested_model, default_model.as_deref()],
                ) {
                    continue;
                }
                let mut candidate = candidate;
                candidate.upstream_model = default_model;
                candidates.push(candidate);
            }
        }
    }

    if candidates.is_empty()
        && requested_model.is_some()
        && route_policy_has_model_restrictions(Some(&route_policy.config))
    {
        return Err(GatewayError::conflict(format!(
            "当前 route policy 不允许模型 {}",
            requested_model.unwrap_or_default()
        )));
    }

    Ok(ResolvedProjectRouteContext {
        route_policy_id: route_policy.id,
        route_policy_config: route_policy.config.clone(),
        selection_strategy: queue_strategy_for_route_policy(&route_policy.config).to_string(),
        candidates,
    })
}

fn queue_strategy_for_route_policy(config: &GatewayRoutePolicyConfig) -> &'static str {
    if config.selection_strategy == "priority" {
        "round_robin"
    } else {
        "priority_weighted"
    }
}
