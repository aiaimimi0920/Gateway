use super::*;

pub async fn list_models_for_project(
    pool: &PgPool,
    project_id: &str,
) -> Result<Vec<ModelInfo>, GatewayError> {
    list_models_for_project_admitted(pool, project_id, &|_, _, _| true).await
}

pub(crate) async fn list_models_for_project_admitted(
    pool: &PgPool,
    project_id: &str,
    admit: &impl Fn(&str, Option<&str>, &str) -> bool,
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

    let provider_rows = fetch_active_provider_rows(pool)
        .await?
        .into_iter()
        .filter(|provider| {
            provider_allowed_by_route_policy(
                provider,
                route_policy.as_ref().map(|policy| &policy.config),
            )
        })
        .collect::<Vec<_>>();
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
    let credentials =
        crate::db::model_catalog_admission::CatalogCredentials::load(pool, &provider_account_ids)
            .await?;
    let mut defaults = HashMap::new();
    for provider_row in &provider_rows {
        if !provider_allowed_by_route_policy(
            provider_row,
            route_policy.as_ref().map(|policy| &policy.config),
        ) {
            continue;
        }
        let Some(account_payload) = provider_payload_value_from_row(provider_row).await? else {
            continue;
        };
        let Some(payload) = provider_payload_from_value(provider_row, account_payload)? else {
            continue;
        };
        defaults.insert(provider_row.id.clone(), payload.default_model);
    }
    let model_ids = super::catalog_selection::collect_catalog_ids(
        route_policy.as_ref().map(|policy| &policy.config),
        &alias_rows,
        &provider_rows,
        &capability_rows,
        &defaults,
        &|provider, model, upstream| {
            credentials.allows(
                provider,
                upstream,
                |credential| {
                    credential_payload_supports_model(
                        &credential.payload,
                        Some(model),
                        Some(upstream),
                    )
                },
                admit,
            )
        },
    );
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
