use super::*;

pub async fn list_route_policies(
    pool: &PgPool,
    project_id: &str,
) -> Result<Vec<GatewayRoutePolicyView>, GatewayError> {
    let rows = sqlx::query_as::<_, GatewayRoutePolicyRow>(
        r#"
        select id, project_id, name, is_default, enabled, config, created_at, updated_at
        from gateway_route_policies
        where project_id = $1
        order by updated_at desc, created_at desc
        "#,
    )
    .bind(project_id)
    .fetch_all(pool)
    .await
    .map_err(map_db_error)?;

    rows.into_iter().map(route_policy_view_from_row).collect()
}

pub async fn save_route_policy(
    pool: &PgPool,
    policy_id: Option<&str>,
    input: SaveRoutePolicyInput,
) -> Result<GatewayRoutePolicyView, GatewayError> {
    get_active_project(pool, &input.project_id).await?;

    let config_json = serde_json::to_value(&input.config).map_err(|error| {
        GatewayError::server_error(format!("serialize route policy config: {error}"))
    })?;
    let mut tx = pool.begin().await.map_err(map_db_error)?;
    let timestamp = OffsetDateTime::now_utc();

    let view = if let Some(policy_id) = policy_id {
        let updated = sqlx::query_as::<_, GatewayRoutePolicyRow>(
            r#"
            update gateway_route_policies
            set
              project_id = $2,
              name = $3,
              is_default = $4,
              enabled = $5,
              config = $6,
              updated_at = $7
            where id = $1
            returning id, project_id, name, is_default, enabled, config, created_at, updated_at
            "#,
        )
        .bind(policy_id)
        .bind(&input.project_id)
        .bind(&input.name)
        .bind(input.is_default)
        .bind(input.enabled)
        .bind(Json(config_json))
        .bind(timestamp)
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_db_error)?
        .ok_or_else(|| GatewayError::not_found("Route policy 不存在"))?;

        if updated.is_default {
            set_project_default_route_policy(&mut tx, &updated.project_id, &updated.id, timestamp)
                .await?;
        }
        route_policy_view_from_row(updated)?
    } else {
        let created = sqlx::query_as::<_, GatewayRoutePolicyRow>(
            r#"
            insert into gateway_route_policies (
              id, project_id, name, is_default, enabled, config, created_at, updated_at
            ) values ($1, $2, $3, $4, $5, $6, $7, $7)
            returning id, project_id, name, is_default, enabled, config, created_at, updated_at
            "#,
        )
        .bind(Uuid::new_v4().to_string())
        .bind(&input.project_id)
        .bind(&input.name)
        .bind(input.is_default)
        .bind(input.enabled)
        .bind(Json(config_json))
        .bind(timestamp)
        .fetch_one(&mut *tx)
        .await
        .map_err(map_db_error)?;

        if created.is_default {
            set_project_default_route_policy(&mut tx, &created.project_id, &created.id, timestamp)
                .await?;
        }
        route_policy_view_from_row(created)?
    };

    bump_all_access_projection_versions(&mut tx, timestamp).await?;
    tx.commit().await.map_err(map_db_error)?;
    Ok(view)
}

pub async fn find_active_route_policy_for_rate_limits(
    pool: &PgPool,
    project_id: &str,
) -> Result<Option<GatewayRoutePolicyView>, GatewayError> {
    let project = get_active_project(pool, project_id).await?;
    let route_policy = get_route_policy_for_models(
        pool,
        &project.id,
        project.default_route_policy_id.as_deref(),
    )
    .await?;
    Ok(route_policy.filter(|policy| policy.enabled))
}

fn route_policy_view_from_row(
    row: GatewayRoutePolicyRow,
) -> Result<GatewayRoutePolicyView, GatewayError> {
    Ok(GatewayRoutePolicyView {
        id: row.id,
        project_id: row.project_id,
        name: row.name,
        is_default: row.is_default,
        enabled: row.enabled,
        config: parse_route_policy_config(&row.config.0)?,
        created_at: format_timestamp(row.created_at),
        updated_at: format_timestamp(row.updated_at),
    })
}

async fn set_project_default_route_policy(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    project_id: &str,
    route_policy_id: &str,
    timestamp: OffsetDateTime,
) -> Result<(), GatewayError> {
    sqlx::query(
        r#"
        update gateway_projects
        set default_route_policy_id = $2, updated_at = $3
        where id = $1
        "#,
    )
    .bind(project_id)
    .bind(route_policy_id)
    .bind(timestamp)
    .execute(&mut **tx)
    .await
    .map_err(map_db_error)?;

    sqlx::query(
        r#"
        update gateway_route_policies
        set is_default = case when id = $2 then true else false end,
            updated_at = $3
        where project_id = $1
        "#,
    )
    .bind(project_id)
    .bind(route_policy_id)
    .bind(timestamp)
    .execute(&mut **tx)
    .await
    .map_err(map_db_error)?;

    Ok(())
}

pub(super) async fn get_route_policy_for_models(
    pool: &PgPool,
    project_id: &str,
    default_route_policy_id: Option<&str>,
) -> Result<Option<GatewayRoutePolicyView>, GatewayError> {
    let row = if let Some(default_route_policy_id) = default_route_policy_id {
        sqlx::query_as::<_, GatewayRoutePolicyRow>(
            r#"
            select id, project_id, name, is_default, enabled, config, created_at, updated_at
            from gateway_route_policies
            where id = $1
            limit 1
            "#,
        )
        .bind(default_route_policy_id)
        .fetch_optional(pool)
        .await
        .map_err(map_db_error)?
    } else {
        sqlx::query_as::<_, GatewayRoutePolicyRow>(
            r#"
            select id, project_id, name, is_default, enabled, config, created_at, updated_at
            from gateway_route_policies
            where project_id = $1 and is_default = true and enabled = true
            limit 1
            "#,
        )
        .bind(project_id)
        .fetch_optional(pool)
        .await
        .map_err(map_db_error)?
    };

    row.map(route_policy_view_from_row).transpose()
}

pub(super) async fn get_active_route_policy_for_requests(
    pool: &PgPool,
    project_id: &str,
    default_route_policy_id: Option<&str>,
) -> Result<GatewayRoutePolicyView, GatewayError> {
    let route_policy = get_route_policy_for_models(pool, project_id, default_route_policy_id)
        .await?
        .ok_or_else(|| {
            GatewayError::conflict("当前 project 尚未配置可用的 AI gateway route policy")
        })?;
    if !route_policy.enabled {
        return Err(GatewayError::conflict(
            "当前 project 尚未配置可用的 AI gateway route policy",
        ));
    }
    Ok(route_policy)
}
