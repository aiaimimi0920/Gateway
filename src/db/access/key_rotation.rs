use super::keys::list_access_key_rows;
use super::projection::clear_access_projection_cache;
use super::views::to_access_key_view;
use super::*;

pub async fn rotate_access_key(
    pool: &PgPool,
    redis_pool: &RedisPool,
    access_key_id: &str,
    api_key_secret: Option<&str>,
) -> Result<GatewayAccessKeyView, GatewayError> {
    let mut tx = pool.begin().await.map_err(map_db_error)?;
    let current = sqlx::query_as::<_, AccessKeyRow>(
        r#"
        select
          id, owner_type, owner_id, resolved_project_id, resolved_tenant_id, key_kind, status, public_key_prefix,
          display_name, external_key, rotated_from_access_key_id, legacy_gateway_api_key_id, legacy_user_credential_id,
          expires_at, last_used_at, metadata, revoked_at, revoke_reason, created_at, updated_at
        from gateway_access_keys
        where id = $1
        for update
        "#,
    )
    .bind(access_key_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(map_db_error)?
    .ok_or_else(|| GatewayError::not_found("access key 不存在"))?;
    let now = now_utc();
    sqlx::query(
        r#"
        update gateway_access_keys
        set
          status = 'revoked',
          revoked_at = $2,
          revoke_reason = 'rotated',
          legacy_gateway_api_key_id = null,
          legacy_user_credential_id = null,
          updated_at = $2
        where id = $1
        "#,
    )
    .bind(access_key_id)
    .bind(now)
    .execute(&mut *tx)
    .await
    .map_err(map_db_error)?;

    let next_id = Uuid::new_v4().to_string();
    let external_key = if current.public_key_prefix == "gw-user" {
        Some(format!("gw-user-{}", Uuid::new_v4().simple()))
    } else {
        None
    };
    sqlx::query(
        r#"
        insert into gateway_access_keys (
          id, owner_type, owner_id, resolved_project_id, resolved_tenant_id, key_kind, status,
          public_key_prefix, external_key, display_name, rotated_from_access_key_id,
          legacy_gateway_api_key_id, legacy_user_credential_id, expires_at, last_used_at, metadata,
          revoked_at, revoke_reason, created_at, updated_at
        ) values (
          $1, $2, $3, $4, $5, $6, 'active', $7, $8, $9, $10, $11, $12, $13, null, $14, null, null, $15, $15
        )
        "#,
    )
    .bind(&next_id)
    .bind(&current.owner_type)
    .bind(&current.owner_id)
    .bind(&current.resolved_project_id)
    .bind(&current.resolved_tenant_id)
    .bind(&current.key_kind)
    .bind(&current.public_key_prefix)
    .bind(external_key)
    .bind(&current.display_name)
    .bind(&current.id)
    .bind(&current.legacy_gateway_api_key_id)
    .bind(&current.legacy_user_credential_id)
    .bind(current.expires_at)
    .bind(current.metadata.clone())
    .bind(now)
    .execute(&mut *tx)
    .await
    .map_err(map_db_error)?;

    sqlx::query(
        r#"
        insert into gateway_access_key_bundle_bindings (access_key_id, bundle_id, created_at)
        select $1, bundle_id, $3
        from gateway_access_key_bundle_bindings
        where access_key_id = $2
        on conflict (access_key_id, bundle_id) do nothing
        "#,
    )
    .bind(&next_id)
    .bind(access_key_id)
    .bind(now)
    .execute(&mut *tx)
    .await
    .map_err(map_db_error)?;

    sqlx::query(
        r#"
        insert into gateway_access_key_balances (
          access_key_id, balance_mode, status, unlimited_until, period_starts_at, period_ends_at,
          total_tokens, remaining_tokens, total_messages, remaining_messages, updated_at
        )
        select
          $1, balance_mode, status, unlimited_until, period_starts_at, period_ends_at,
          total_tokens, remaining_tokens, total_messages, remaining_messages, $3
        from gateway_access_key_balances
        where access_key_id = $2
        "#,
    )
    .bind(&next_id)
    .bind(access_key_id)
    .bind(now)
    .execute(&mut *tx)
    .await
    .map_err(map_db_error)?;

    sqlx::query(
        r#"
        insert into gateway_access_key_aggregate_memberships (
          aggregate_access_key_id, member_access_key_id, priority, created_at
        )
        select $1, member_access_key_id, priority, $3
        from gateway_access_key_aggregate_memberships
        where aggregate_access_key_id = $2
        on conflict (aggregate_access_key_id, member_access_key_id) do nothing
        "#,
    )
    .bind(&next_id)
    .bind(access_key_id)
    .bind(now)
    .execute(&mut *tx)
    .await
    .map_err(map_db_error)?;

    sqlx::query(
        r#"
        insert into gateway_access_key_aggregate_memberships (
          aggregate_access_key_id, member_access_key_id, priority, created_at
        )
        select aggregate_access_key_id, $1, priority, $3
        from gateway_access_key_aggregate_memberships
        where member_access_key_id = $2
        on conflict (aggregate_access_key_id, member_access_key_id) do nothing
        "#,
    )
    .bind(&next_id)
    .bind(access_key_id)
    .bind(now)
    .execute(&mut *tx)
    .await
    .map_err(map_db_error)?;

    crate::cash_billing::store::rotate(
        &mut crate::cash_billing::sql::CashConnection::Postgres(&mut tx),
        access_key_id,
        &next_id,
    )
    .await?;
    bump_all_access_projection_versions(&mut tx, now).await?;
    tx.commit().await.map_err(map_db_error)?;

    clear_access_projection_cache(redis_pool).await?;
    let row = list_access_key_rows(pool, &next_id).await?;
    to_access_key_view(row, api_key_secret)
}
