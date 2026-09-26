use super::projection::clear_access_projection_cache;
use super::views::{to_platform_access_view, to_provider_capability_view};
use super::*;

pub async fn save_provider_capability(
    pool: &PgPool,
    redis_pool: &RedisPool,
    provider_capability_id: Option<&str>,
    input: UpsertProviderCapabilityInput,
) -> Result<GatewayProviderCapabilityView, GatewayError> {
    let mut tx = pool.begin().await.map_err(map_db_error)?;
    let now = now_utc();
    let row = if let Some(id) = provider_capability_id {
        sqlx::query_as::<_, ProviderCapabilityRow>(
            r#"
            update gateway_provider_capability_catalog
            set
              provider_account_id = $2,
              model_code = $3,
              endpoint_kind = $4,
              upstream_model = $5,
              enabled = $6,
              updated_at = $7
            where id = $1
            returning id, provider_account_id, model_code, endpoint_kind, upstream_model, enabled, created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(input.provider_account_id.trim())
        .bind(input.model_code.trim())
        .bind(input.endpoint_kind.trim())
        .bind(input.upstream_model.as_deref())
        .bind(input.enabled)
        .bind(now)
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_db_error)?
        .ok_or_else(|| GatewayError::not_found("provider capability 不存在"))?
    } else {
        sqlx::query_as::<_, ProviderCapabilityRow>(
            r#"
            insert into gateway_provider_capability_catalog (
              id, provider_account_id, model_code, endpoint_kind, upstream_model, enabled, created_at, updated_at
            ) values ($1, $2, $3, $4, $5, $6, $7, $7)
            returning id, provider_account_id, model_code, endpoint_kind, upstream_model, enabled, created_at, updated_at
            "#,
        )
        .bind(Uuid::new_v4().to_string())
        .bind(input.provider_account_id.trim())
        .bind(input.model_code.trim())
        .bind(input.endpoint_kind.trim())
        .bind(input.upstream_model.as_deref())
        .bind(input.enabled)
        .bind(now)
        .fetch_one(&mut *tx)
        .await
        .map_err(map_db_error)?
    };
    bump_all_access_projection_versions(&mut tx, now).await?;
    tx.commit().await.map_err(map_db_error)?;
    clear_access_projection_cache(redis_pool).await?;
    Ok(to_provider_capability_view(row))
}

pub async fn save_platform_access(
    pool: &PgPool,
    redis_pool: &RedisPool,
    platform_access_id: Option<&str>,
    input: UpsertPlatformAccessInput,
) -> Result<GatewayPlatformAccessView, GatewayError> {
    let mut tx = pool.begin().await.map_err(map_db_error)?;
    let now = now_utc();
    let row = if let Some(id) = platform_access_id {
        sqlx::query_as::<_, PlatformAccessRow>(
            r#"
            update gateway_platform_access_catalog
            set
              provider_capability_id = $2,
              model_code = $3,
              endpoint_kind = $4,
              upstream_model = $5,
              platform_tier = $6,
              status = $7,
              operator_weight = $8,
              routing_priority = $9,
              enabled_for_sale = $10,
              notes = $11,
              updated_at = $12
            where id = $1
            returning
              id, provider_capability_id,
              (select provider_account_id from gateway_provider_capability_catalog where id = provider_capability_id) as provider_account_id,
              model_code, endpoint_kind, upstream_model, platform_tier, status, operator_weight, routing_priority, enabled_for_sale,
              notes, created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(input.provider_capability_id.trim())
        .bind(input.model_code.trim())
        .bind(input.endpoint_kind.trim())
        .bind(input.upstream_model.as_deref())
        .bind(normalize_platform_tier(&input.platform_tier))
        .bind(normalize_status(&input.status))
        .bind(input.operator_weight)
        .bind(input.routing_priority)
        .bind(input.enabled_for_sale)
        .bind(input.notes.as_deref())
        .bind(now)
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_db_error)?
        .ok_or_else(|| GatewayError::not_found("platform access 不存在"))?
    } else {
        sqlx::query_as::<_, PlatformAccessRow>(
            r#"
            insert into gateway_platform_access_catalog (
              id, provider_capability_id, model_code, endpoint_kind, upstream_model, platform_tier, status,
              operator_weight, routing_priority, enabled_for_sale, notes, created_at, updated_at
            ) values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $12)
            returning
              id, provider_capability_id,
              (select provider_account_id from gateway_provider_capability_catalog where id = provider_capability_id) as provider_account_id,
              model_code, endpoint_kind, upstream_model, platform_tier, status, operator_weight, routing_priority, enabled_for_sale,
              notes, created_at, updated_at
            "#,
        )
        .bind(Uuid::new_v4().to_string())
        .bind(input.provider_capability_id.trim())
        .bind(input.model_code.trim())
        .bind(input.endpoint_kind.trim())
        .bind(input.upstream_model.as_deref())
        .bind(normalize_platform_tier(&input.platform_tier))
        .bind(normalize_status(&input.status))
        .bind(input.operator_weight)
        .bind(input.routing_priority)
        .bind(input.enabled_for_sale)
        .bind(input.notes.as_deref())
        .bind(now)
        .fetch_one(&mut *tx)
        .await
        .map_err(map_db_error)?
    };
    bump_all_access_projection_versions(&mut tx, now).await?;
    tx.commit().await.map_err(map_db_error)?;
    clear_access_projection_cache(redis_pool).await?;
    Ok(to_platform_access_view(row))
}
