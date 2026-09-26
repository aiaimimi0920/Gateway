use super::*;

pub async fn list_model_aliases(
    pool: &PgPool,
    project_id: Option<&str>,
) -> Result<Vec<GatewayModelAliasView>, GatewayError> {
    let rows = if let Some(project_id) = project_id {
        sqlx::query_as::<_, GatewayModelAliasRow>(
            r#"
            select id, project_id, scope_type, alias, provider_account_id, upstream_model, priority, weight, enabled, created_at, updated_at
            from gateway_model_aliases
            where project_id = $1 or project_id is null
            order by alias asc, priority asc, weight desc, created_at asc
            "#,
        )
        .bind(project_id)
        .fetch_all(pool)
        .await
        .map_err(map_db_error)?
    } else {
        sqlx::query_as::<_, GatewayModelAliasRow>(
            r#"
            select id, project_id, scope_type, alias, provider_account_id, upstream_model, priority, weight, enabled, created_at, updated_at
            from gateway_model_aliases
            order by alias asc, priority asc, weight desc, created_at asc
            "#,
        )
        .fetch_all(pool)
        .await
        .map_err(map_db_error)?
    };

    Ok(rows.into_iter().map(model_alias_view_from_row).collect())
}

pub async fn save_model_alias(
    pool: &PgPool,
    alias_id: Option<&str>,
    input: SaveModelAliasInput,
) -> Result<GatewayModelAliasView, GatewayError> {
    let mut tx = pool.begin().await.map_err(map_db_error)?;
    let timestamp = OffsetDateTime::now_utc();
    let project_id = input.project_id.as_deref();

    let row = if let Some(alias_id) = alias_id {
        sqlx::query_as::<_, GatewayModelAliasRow>(
            r#"
            update gateway_model_aliases
            set
              project_id = $2,
              scope_type = $3,
              alias = $4,
              provider_account_id = $5,
              upstream_model = $6,
              priority = $7,
              weight = $8,
              enabled = $9,
              updated_at = $10
            where id = $1
            returning id, project_id, scope_type, alias, provider_account_id, upstream_model, priority, weight, enabled, created_at, updated_at
            "#,
        )
        .bind(alias_id)
        .bind(project_id)
        .bind(&input.scope_type)
        .bind(&input.alias)
        .bind(&input.provider_account_id)
        .bind(input.upstream_model.as_deref())
        .bind(input.priority)
        .bind(input.weight)
        .bind(input.enabled)
        .bind(timestamp)
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_db_error)?
        .ok_or_else(|| GatewayError::not_found("模型关联不存在"))?
    } else {
        sqlx::query_as::<_, GatewayModelAliasRow>(
            r#"
            insert into gateway_model_aliases (
              id, project_id, scope_type, alias, provider_account_id, upstream_model, priority, weight, enabled, created_at, updated_at
            ) values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $10)
            returning id, project_id, scope_type, alias, provider_account_id, upstream_model, priority, weight, enabled, created_at, updated_at
            "#,
        )
        .bind(Uuid::new_v4().to_string())
        .bind(project_id)
        .bind(&input.scope_type)
        .bind(&input.alias)
        .bind(&input.provider_account_id)
        .bind(input.upstream_model.as_deref())
        .bind(input.priority)
        .bind(input.weight)
        .bind(input.enabled)
        .bind(timestamp)
        .fetch_one(&mut *tx)
        .await
        .map_err(map_db_error)?
    };

    bump_all_access_projection_versions(&mut tx, timestamp).await?;
    tx.commit().await.map_err(map_db_error)?;
    Ok(model_alias_view_from_row(row))
}

pub async fn delete_model_alias(
    pool: &PgPool,
    alias_id: &str,
) -> Result<GatewayModelAliasView, GatewayError> {
    let mut tx = pool.begin().await.map_err(map_db_error)?;
    let row = sqlx::query_as::<_, GatewayModelAliasRow>(
        r#"
        delete from gateway_model_aliases
        where id = $1
        returning id, project_id, scope_type, alias, provider_account_id, upstream_model, priority, weight, enabled, created_at, updated_at
        "#,
    )
    .bind(alias_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(map_db_error)?
    .ok_or_else(|| GatewayError::not_found("模型关联不存在"))?;

    bump_all_access_projection_versions(&mut tx, OffsetDateTime::now_utc()).await?;
    tx.commit().await.map_err(map_db_error)?;
    Ok(model_alias_view_from_row(row))
}

fn model_alias_view_from_row(row: GatewayModelAliasRow) -> GatewayModelAliasView {
    GatewayModelAliasView {
        id: row.id,
        project_id: row.project_id,
        scope_type: row.scope_type,
        alias: row.alias,
        provider_account_id: row.provider_account_id,
        upstream_model: row.upstream_model,
        priority: row.priority,
        weight: row.weight,
        enabled: row.enabled,
        created_at: format_timestamp(row.created_at),
        updated_at: format_timestamp(row.updated_at),
    }
}

pub(super) async fn fetch_alias_rows(
    pool: &PgPool,
    project_id: &str,
    requested_model: Option<&str>,
) -> Result<Vec<GatewayModelAliasRow>, GatewayError> {
    if let Some(requested_model) = requested_model {
        sqlx::query_as::<_, GatewayModelAliasRow>(
            r#"
            select id, project_id, scope_type, alias, provider_account_id, upstream_model, priority, weight, enabled, created_at, updated_at
            from gateway_model_aliases
            where enabled = true
              and alias = $2
              and (project_id = $1 or project_id is null)
            order by priority asc, weight desc, created_at asc
            "#,
        )
        .bind(project_id)
        .bind(requested_model)
        .fetch_all(pool)
        .await
        .map_err(map_db_error)
    } else {
        sqlx::query_as::<_, GatewayModelAliasRow>(
            r#"
            select id, project_id, scope_type, alias, provider_account_id, upstream_model, priority, weight, enabled, created_at, updated_at
            from gateway_model_aliases
            where enabled = true
              and (project_id = $1 or project_id is null)
            order by priority asc, weight desc, created_at asc
            "#,
        )
        .bind(project_id)
        .fetch_all(pool)
        .await
        .map_err(map_db_error)
    }
}
