//! Benefit tenant/project provisioning and transactional default routing.

use super::*;

pub async fn ensure_benefit_project(
    pool: &PgPool,
    service_id: &str,
    user_id: &str,
    service_title: Option<&str>,
) -> Result<GatewayBenefitProjectEnsureView, GatewayError> {
    let normalized_service_id = service_id.trim();
    let normalized_user_id = user_id.trim();
    if normalized_service_id.is_empty() {
        return Err(GatewayError::bad_request("serviceId 不能为空"));
    }
    if normalized_user_id.is_empty() {
        return Err(GatewayError::bad_request("userId 不能为空"));
    }

    let mut tx = pool.begin().await.map_err(map_db_error)?;
    let now = OffsetDateTime::now_utc();
    let tenant_source_key = build_benefit_tenant_source_key(normalized_user_id);
    let tenant = if let Some(existing) = sqlx::query_as::<_, GatewayTenantDetailRow>(
        r#"
        select id, slug, display_name, status, owner_user_id, source_kind, source_key, created_at, updated_at
        from gateway_tenants
        where source_kind = 'benefit_user' and source_key = $1
        limit 1
        "#,
    )
    .bind(&tenant_source_key)
    .fetch_optional(&mut *tx)
    .await
    .map_err(map_db_error)?
    {
        existing
    } else {
        sqlx::query_as::<_, GatewayTenantDetailRow>(
            r#"
            insert into gateway_tenants (
              id,
              slug,
              display_name,
              status,
              owner_user_id,
              source_kind,
              source_key,
              created_at,
              updated_at
            ) values ($1, $2, $3, 'active', $4, 'benefit_user', $5, $6, $6)
            returning id, slug, display_name, status, owner_user_id, source_kind, source_key, created_at, updated_at
            "#,
        )
        .bind(Uuid::new_v4().to_string())
        .bind(slugify_identifier(&format!("benefit-{normalized_user_id}")))
        .bind(format!("Benefit {normalized_user_id}"))
        .bind(normalized_user_id)
        .bind(&tenant_source_key)
        .bind(now)
        .fetch_one(&mut *tx)
        .await
        .map_err(map_db_error)?
    };

    let project_source_key =
        build_benefit_project_source_key(normalized_service_id, normalized_user_id);
    let service_display_name = service_title
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| normalized_service_id);
    let project = if let Some(existing) = sqlx::query_as::<_, GatewayProjectDetailRow>(
        r#"
        select
          id,
          tenant_id,
          slug,
          display_name,
          status,
          source_kind,
          source_key,
          default_route_policy_id,
          created_at,
          updated_at
        from gateway_projects
        where source_kind = 'benefit_service_user' and source_key = $1
        limit 1
        "#,
    )
    .bind(&project_source_key)
    .fetch_optional(&mut *tx)
    .await
    .map_err(map_db_error)?
    {
        existing
    } else {
        sqlx::query_as::<_, GatewayProjectDetailRow>(
            r#"
            insert into gateway_projects (
              id,
              tenant_id,
              slug,
              display_name,
              status,
              source_kind,
              source_key,
              default_route_policy_id,
              created_at,
              updated_at
            ) values ($1, $2, $3, $4, 'active', 'benefit_service_user', $5, null, $6, $6)
            returning
              id,
              tenant_id,
              slug,
              display_name,
              status,
              source_kind,
              source_key,
              default_route_policy_id,
              created_at,
              updated_at
            "#,
        )
        .bind(Uuid::new_v4().to_string())
        .bind(&tenant.id)
        .bind(slugify_identifier(&format!(
            "{service_display_name}-{normalized_user_id}"
        )))
        .bind(service_display_name)
        .bind(&project_source_key)
        .bind(now)
        .fetch_one(&mut *tx)
        .await
        .map_err(map_db_error)?
    };

    let default_route_policy_id =
        ensure_default_route_policy_in_tx(&mut tx, &project.id, now).await?;
    tx.commit().await.map_err(map_db_error)?;

    let project = get_project_detail(pool, &project.id)
        .await?
        .ok_or_else(|| GatewayError::not_found("AI gateway project 不存在"))?;
    let route_policy = list_route_policies(pool, &project.id)
        .await?
        .into_iter()
        .find(|policy| policy.id == default_route_policy_id)
        .ok_or_else(|| GatewayError::conflict("当前 benefit project 尚未生成默认 route policy"))?;

    Ok(GatewayBenefitProjectEnsureView {
        tenant: gateway_tenant_view_from_row(tenant),
        project: gateway_project_view_from_row(project),
        route_policy,
    })
}

fn build_benefit_tenant_source_key(user_id: &str) -> String {
    format!("benefit_user:{user_id}")
}

fn build_benefit_project_source_key(service_id: &str, user_id: &str) -> String {
    format!("benefit_service_user:{service_id}:{user_id}")
}

fn slugify_identifier(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut last_dash = false;
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash && !out.is_empty() {
            out.push('-');
            last_dash = true;
        }
    }
    let normalized = out.trim_matches('-').to_string();
    if normalized.is_empty() {
        "gateway-item".to_string()
    } else {
        normalized
    }
}

async fn ensure_default_route_policy_in_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    project_id: &str,
    timestamp: OffsetDateTime,
) -> Result<String, GatewayError> {
    let existing = sqlx::query(
        r#"
        select id
        from gateway_route_policies
        where project_id = $1 and is_default = true
        limit 1
        "#,
    )
    .bind(project_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(map_db_error)?;

    let route_policy_id = if let Some(row) = existing {
        row.try_get::<String, _>("id")
            .map_err(map_db_decode_error)?
    } else {
        let route_policy_id = Uuid::new_v4().to_string();
        let config = normalize_route_policy_config(GatewayRoutePolicyConfigInput::default())?;
        let config_json = serde_json::to_value(config).map_err(|error| {
            GatewayError::server_error(format!("serialize default route policy config: {error}"))
        })?;
        sqlx::query(
            r#"
            insert into gateway_route_policies (
              id,
              project_id,
              name,
              is_default,
              enabled,
              config,
              created_at,
              updated_at
            ) values ($1, $2, 'default', true, true, $3, $4, $4)
            "#,
        )
        .bind(&route_policy_id)
        .bind(project_id)
        .bind(Json(config_json))
        .bind(timestamp)
        .execute(&mut **tx)
        .await
        .map_err(map_db_error)?;
        route_policy_id
    };

    sqlx::query(
        r#"
        update gateway_projects
        set default_route_policy_id = $2, updated_at = $3
        where id = $1 and (default_route_policy_id is null or default_route_policy_id <> $2)
        "#,
    )
    .bind(project_id)
    .bind(&route_policy_id)
    .bind(timestamp)
    .execute(&mut **tx)
    .await
    .map_err(map_db_error)?;

    Ok(route_policy_id)
}
