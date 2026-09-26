use super::projection::clear_access_projection_cache;
use super::views::to_aggregate_membership_view;
use super::*;

pub async fn replace_access_key_aggregate_memberships(
    pool: &PgPool,
    redis_pool: &RedisPool,
    aggregate_access_key_id: &str,
    memberships: &[AggregateMembershipInput],
) -> Result<Vec<GatewayAccessKeyAggregateMembershipView>, GatewayError> {
    let mut tx = pool.begin().await.map_err(map_db_error)?;
    let memberships =
        validate_aggregate_membership_boundaries(&mut tx, aggregate_access_key_id, memberships)
            .await?;
    sqlx::query(
        "delete from gateway_access_key_aggregate_memberships where aggregate_access_key_id = $1",
    )
    .bind(aggregate_access_key_id)
    .execute(&mut *tx)
    .await
    .map_err(map_db_error)?;
    let now = now_utc();
    for membership in &memberships {
        sqlx::query(
            r#"
            insert into gateway_access_key_aggregate_memberships (
              aggregate_access_key_id, member_access_key_id, priority, created_at
            ) values ($1, $2, $3, $4)
            "#,
        )
        .bind(aggregate_access_key_id)
        .bind(membership.member_access_key_id.trim())
        .bind(membership.priority)
        .bind(now)
        .execute(&mut *tx)
        .await
        .map_err(map_db_error)?;
    }
    bump_all_access_projection_versions(&mut tx, now).await?;
    tx.commit().await.map_err(map_db_error)?;
    clear_access_projection_cache(redis_pool).await?;
    let rows = sqlx::query_as::<_, AggregateMembershipRow>(
        r#"
        select aggregate_access_key_id, member_access_key_id, priority, created_at
        from gateway_access_key_aggregate_memberships
        where aggregate_access_key_id = $1
        order by priority asc, member_access_key_id asc
        "#,
    )
    .bind(aggregate_access_key_id)
    .fetch_all(pool)
    .await
    .map_err(map_db_error)?;
    Ok(rows.into_iter().map(to_aggregate_membership_view).collect())
}

pub(super) async fn ensure_project_tenant_boundary(
    tx: &mut Transaction<'_, Postgres>,
    expected: &AccessBoundary,
) -> Result<(), GatewayError> {
    let actual_tenant_id = sqlx::query_scalar::<_, String>(
        r#"
        select project.tenant_id
        from gateway_projects project
        join gateway_tenants tenant on tenant.id = project.tenant_id
        where project.id = $1
          and project.status = 'active'
          and tenant.status = 'active'
        limit 1
        "#,
    )
    .bind(&expected.project_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(map_db_error)?
    .ok_or_else(|| {
        GatewayError::bad_request("resolved project is not active")
            .with_code("access_boundary_mismatch")
    })?;
    let actual = AccessBoundary::new(&actual_tenant_id, &expected.project_id)?;
    expected.ensure_same(&actual, "access key")
}

async fn validate_aggregate_membership_boundaries(
    tx: &mut Transaction<'_, Postgres>,
    aggregate_access_key_id: &str,
    memberships: &[AggregateMembershipInput],
) -> Result<Vec<AggregateMembershipInput>, GatewayError> {
    let aggregate = sqlx::query(
        r#"
        select key_kind, status
        from gateway_access_keys
        where id = $1
        limit 1
        "#,
    )
    .bind(aggregate_access_key_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(map_db_error)?
    .ok_or_else(|| GatewayError::not_found("aggregate access key does not exist"))?;
    if normalize_key_kind(aggregate.get::<String, _>("key_kind").as_str()) != "auto_route"
        || normalize_status(aggregate.get::<String, _>("status").as_str()) != "active"
    {
        return Err(GatewayError::bad_request(
            "aggregate access key must be an active auto_route key",
        )
        .with_code("access_boundary_mismatch"));
    }

    let requested_ids = memberships
        .iter()
        .map(|membership| membership.member_access_key_id.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    let allowed_ids = if requested_ids.is_empty() {
        Vec::new()
    } else {
        sqlx::query_scalar::<_, String>(
            r#"
            select member.id
            from gateway_access_keys member
            join gateway_access_keys aggregate on aggregate.id = $1
            where member.id = any($2)
              and member.id <> aggregate.id
              and member.status = 'active'
              and member.resolved_project_id = aggregate.resolved_project_id
              and member.resolved_tenant_id = aggregate.resolved_tenant_id
            order by member.id asc
            "#,
        )
        .bind(aggregate_access_key_id)
        .bind(&requested_ids)
        .fetch_all(&mut **tx)
        .await
        .map_err(map_db_error)?
    };
    let allowed_ids =
        ensure_allowed_resource_ids(&requested_ids, &allowed_ids, "aggregate access key member")?;
    let priorities = memberships
        .iter()
        .map(|membership| (membership.member_access_key_id.trim(), membership.priority))
        .collect::<HashMap<_, _>>();

    Ok(allowed_ids
        .into_iter()
        .map(|member_access_key_id| AggregateMembershipInput {
            priority: priorities
                .get(member_access_key_id.as_str())
                .copied()
                .unwrap_or_default(),
            member_access_key_id,
        })
        .collect())
}
