use super::*;

pub async fn list_models_for_project(
    pool: &PgPool,
    project_id: &str,
) -> Result<Vec<ModelInfo>, GatewayError> {
    let project = get_active_project(pool, project_id).await?;
    let route_policy = get_route_policy_for_models(
        pool,
        &project.id,
        project.default_route_policy_id.as_deref(),
    )
    .await?;
    let alias_rows = sqlx::query_as::<_, GatewayModelAliasRow>(
        r#"
        select id, project_id, scope_type, alias, provider_account_id, upstream_model, priority, weight, enabled, created_at, updated_at
        from gateway_model_aliases
        where project_id = $1 or project_id is null
        order by alias asc, priority asc, weight desc, created_at asc
        "#,
    )
    .bind(&project.id)
    .fetch_all(pool)
    .await
    .map_err(map_db_error)?;

    let provider_rows = fetch_active_provider_rows(pool).await?;
    let provider_rows_by_id = provider_rows
        .iter()
        .map(|row| (row.id.as_str(), row))
        .collect::<HashMap<_, _>>();
    let provider_account_ids = provider_rows
        .iter()
        .map(|row| row.id.clone())
        .collect::<Vec<_>>();
    let capability_rows = if provider_account_ids.is_empty() {
        Vec::new()
    } else {
        sqlx::query_as::<_, ProviderCapabilityModelRow>(
            r#"
            select
              provider_account_id,
              model_code,
              upstream_model
            from gateway_provider_capability_catalog
            where enabled = true
              and provider_account_id = any($1)
            group by provider_account_id, model_code, upstream_model
            order by provider_account_id asc, model_code asc
            "#,
        )
        .bind(&provider_account_ids)
        .fetch_all(pool)
        .await
        .map_err(map_db_error)?
    };
    let shadowed_upstream_models = collect_shadowed_upstream_models_for_catalog(
        route_policy.as_ref().map(|policy| &policy.config),
        &alias_rows,
        &capability_rows,
        &provider_rows_by_id,
    );
    let mut model_ids = BTreeSet::new();

    for row in alias_rows {
        if row.enabled
            && route_policy_allows_models(
                route_policy.as_ref().map(|policy| &policy.config),
                [Some(row.alias.as_str()), row.upstream_model.as_deref()],
            )
        {
            model_ids.insert(row.alias);
        }
    }

    for row in capability_rows {
        let Some(provider_row) = provider_rows_by_id.get(row.provider_account_id.as_str()) else {
            continue;
        };
        if !provider_allowed_by_route_policy(
            provider_row,
            route_policy.as_ref().map(|policy| &policy.config),
        ) {
            continue;
        }
        if !route_policy_allows_models(
            route_policy.as_ref().map(|policy| &policy.config),
            [Some(row.model_code.as_str()), row.upstream_model.as_deref()],
        ) {
            continue;
        }
        let upstream_model = row
            .upstream_model
            .as_deref()
            .unwrap_or(row.model_code.as_str());
        if row.model_code == upstream_model
            && shadowed_upstream_models
                .contains(&(row.provider_account_id.clone(), upstream_model.to_string()))
        {
            continue;
        }
        model_ids.insert(row.model_code.clone());
    }

    for provider_row in provider_rows {
        if !provider_allowed_by_route_policy(
            &provider_row,
            route_policy.as_ref().map(|policy| &policy.config),
        ) {
            continue;
        }
        let Some(account_payload) = provider_payload_value_from_row(&provider_row).await? else {
            continue;
        };
        let Some(payload) = provider_payload_from_value(&provider_row, account_payload)? else {
            continue;
        };
        if let Some(default_model) = payload.default_model.as_deref() {
            if route_policy_allows_models(
                route_policy.as_ref().map(|policy| &policy.config),
                [Some(default_model), None],
            ) && !shadowed_upstream_models
                .contains(&(provider_row.id.clone(), default_model.to_string()))
            {
                model_ids.insert(default_model.to_string());
            }
        }
    }

    let created = OffsetDateTime::now_utc().unix_timestamp();
    Ok(model_ids
        .into_iter()
        .map(|id| ModelInfo {
            id,
            object: "model".to_string(),
            created,
            owned_by: "neuro-gateway".to_string(),
        })
        .collect())
}

pub(super) fn collect_shadowed_upstream_models_for_catalog(
    route_policy: Option<&GatewayRoutePolicyConfig>,
    alias_rows: &[GatewayModelAliasRow],
    capability_rows: &[ProviderCapabilityModelRow],
    provider_rows_by_id: &HashMap<&str, &GatewayProviderRouteRow>,
) -> HashSet<(String, String)> {
    let mut shadowed = HashSet::new();

    for row in alias_rows {
        let Some(upstream_model) = row.upstream_model.as_deref() else {
            continue;
        };
        if !row.enabled || row.alias == upstream_model {
            continue;
        }
        if !route_policy_allows_models(
            route_policy,
            [Some(row.alias.as_str()), Some(upstream_model)],
        ) {
            continue;
        }
        shadowed.insert((row.provider_account_id.clone(), upstream_model.to_string()));
    }

    for row in capability_rows {
        let Some(provider_row) = provider_rows_by_id.get(row.provider_account_id.as_str()) else {
            continue;
        };
        let Some(upstream_model) = row.upstream_model.as_deref() else {
            continue;
        };
        if row.model_code == upstream_model {
            continue;
        }
        if !provider_allowed_by_route_policy(provider_row, route_policy) {
            continue;
        }
        if !route_policy_allows_models(
            route_policy,
            [Some(row.model_code.as_str()), Some(upstream_model)],
        ) {
            continue;
        }
        shadowed.insert((row.provider_account_id.clone(), upstream_model.to_string()));
    }

    shadowed
}
