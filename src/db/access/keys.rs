use super::memberships::ensure_project_tenant_boundary;
use super::projection::clear_access_projection_cache;
use super::views::to_access_key_view;
use super::*;

pub(super) fn build_access_key_token(
    row: &AccessKeyRow,
    api_key_secret: Option<&str>,
) -> Result<Option<String>, GatewayError> {
    match row.public_key_prefix.as_str() {
        "gw-user" => Ok(row.external_key.clone()),
        prefix
            if prefix == "neuro"
                || prefix == "new_api"
                || normalize_bundle_scoped_gateway_project_api_key_prefix(prefix).is_some() =>
        {
            let secret = api_key_secret
                .ok_or_else(|| GatewayError::conflict("当前环境尚未配置 GATEWAY_API_KEY_SECRET"))?;
            Ok(Some(build_gateway_project_api_key_with_prefix(
                &row.id,
                &row.resolved_project_id,
                &row.resolved_tenant_id,
                secret,
                prefix,
            )))
        }
        _ => Ok(row.external_key.clone()),
    }
}

fn normalize_public_key_prefix(owner_type: &str, raw_prefix: &str) -> String {
    let normalized = raw_prefix.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "gw-user" => "gw-user".to_string(),
        "new_api" | "neuro" => "neuro".to_string(),
        _ => normalize_bundle_scoped_gateway_project_api_key_prefix(raw_prefix).unwrap_or_else(
            || {
                if owner_type == "platform" || owner_type == "project" {
                    "neuro".to_string()
                } else {
                    raw_prefix.trim().to_string()
                }
            },
        ),
    }
}

fn normalize_owner_type(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "platform" => "platform".to_string(),
        "project" => "project".to_string(),
        _ => "user".to_string(),
    }
}

async fn resolve_bundle_ids_for_key(
    tx: &mut Transaction<'_, Postgres>,
    resolved_project_id: &str,
    explicit_bundle_ids: &[String],
) -> Result<Vec<String>, GatewayError> {
    if !explicit_bundle_ids.is_empty() {
        let requested_ids = explicit_bundle_ids
            .iter()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>();
        let allowed_ids = sqlx::query_scalar::<_, String>(
            r#"
            select id
            from gateway_access_bundles
            where id = any($1)
              and status = 'active'
              and (project_id = $2 or project_id is null)
            order by id asc
            "#,
        )
        .bind(&requested_ids)
        .bind(resolved_project_id)
        .fetch_all(&mut **tx)
        .await
        .map_err(map_db_error)?;
        return ensure_allowed_resource_ids(&requested_ids, &allowed_ids, "access bundle");
    }
    sqlx::query_scalar::<_, String>(
        r#"
        select id
        from gateway_access_bundles
        where project_id = $1 and status = 'active'
        order by created_at asc, id asc
        "#,
    )
    .bind(resolved_project_id)
    .fetch_all(&mut **tx)
    .await
    .map_err(map_db_error)
}

pub(super) async fn list_access_key_rows(
    pool: &PgPool,
    access_key_id: &str,
) -> Result<AccessKeyRow, GatewayError> {
    sqlx::query_as::<_, AccessKeyRow>(
        r#"
        select
          id, owner_type, owner_id, resolved_project_id, resolved_tenant_id, key_kind, status, public_key_prefix,
          display_name, external_key, rotated_from_access_key_id, legacy_gateway_api_key_id, legacy_user_credential_id,
          expires_at, last_used_at, metadata, revoked_at, revoke_reason, created_at, updated_at
        from gateway_access_keys
        where id = $1
        limit 1
        "#,
    )
    .bind(access_key_id)
    .fetch_optional(pool)
    .await
    .map_err(map_db_error)?
    .ok_or_else(|| GatewayError::not_found("access key 不存在"))
}

pub async fn save_access_key(
    pool: &PgPool,
    redis_pool: &RedisPool,
    access_key_id: Option<&str>,
    api_key_secret: Option<&str>,
    input: UpsertAccessKeyInput,
) -> Result<GatewayAccessKeyView, GatewayError> {
    save_access_key_with_quota(pool, redis_pool, access_key_id, api_key_secret, input, None).await
}

pub async fn save_access_key_with_quota(
    pool: &PgPool,
    redis_pool: &RedisPool,
    access_key_id: Option<&str>,
    api_key_secret: Option<&str>,
    input: UpsertAccessKeyInput,
    quota: Option<&crate::access_balance::quota::KeyQuotaInput>,
) -> Result<GatewayAccessKeyView, GatewayError> {
    if let Some(quota) = quota {
        quota.validate()?;
    }
    let boundary = AccessBoundary::new(&input.resolved_tenant_id, &input.resolved_project_id)?;
    let mut tx = pool.begin().await.map_err(map_db_error)?;
    ensure_project_tenant_boundary(&mut tx, &boundary).await?;
    let bundle_ids =
        resolve_bundle_ids_for_key(&mut tx, &input.resolved_project_id, &input.bundle_ids).await?;
    let now = now_utc();
    let key_kind = normalize_key_kind(&input.key_kind);
    let owner_type = normalize_owner_type(&input.owner_type);
    let public_key_prefix = normalize_public_key_prefix(&owner_type, &input.public_key_prefix);
    let expires_at = parse_optional_timestamp(input.expires_at.as_deref());
    let access_key_id = if let Some(id) = access_key_id {
        sqlx::query(
            r#"
            update gateway_access_keys
            set
              owner_type = $2,
              owner_id = $3,
              resolved_project_id = $4,
              resolved_tenant_id = $5,
              key_kind = $6,
              public_key_prefix = $7,
              display_name = $8,
              expires_at = $9,
              metadata = $10,
              updated_at = $11
            where id = $1
            "#,
        )
        .bind(id)
        .bind(&owner_type)
        .bind(input.owner_id.trim())
        .bind(input.resolved_project_id.trim())
        .bind(input.resolved_tenant_id.trim())
        .bind(&key_kind)
        .bind(&public_key_prefix)
        .bind(input.display_name.trim())
        .bind(expires_at)
        .bind(input.metadata.clone().map(Json))
        .bind(now)
        .execute(&mut *tx)
        .await
        .map_err(map_db_error)?;
        id.to_string()
    } else {
        let id = Uuid::new_v4().to_string();
        let external_key = if public_key_prefix == "gw-user" {
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
              $1, $2, $3, $4, $5, $6, 'active', $7, $8, $9, null, null, null, $10, null, $11, null, null, $12, $12
            )
            "#,
        )
        .bind(&id)
        .bind(&owner_type)
        .bind(input.owner_id.trim())
        .bind(input.resolved_project_id.trim())
        .bind(input.resolved_tenant_id.trim())
        .bind(&key_kind)
        .bind(&public_key_prefix)
        .bind(external_key)
        .bind(input.display_name.trim())
        .bind(expires_at)
        .bind(input.metadata.clone().map(Json))
        .bind(now)
        .execute(&mut *tx)
        .await
        .map_err(map_db_error)?;
        id
    };

    sqlx::query("delete from gateway_access_key_bundle_bindings where access_key_id = $1")
        .bind(&access_key_id)
        .execute(&mut *tx)
        .await
        .map_err(map_db_error)?;
    if key_kind != "auto_route" {
        for bundle_id in &bundle_ids {
            sqlx::query(
                r#"
                insert into gateway_access_key_bundle_bindings (access_key_id, bundle_id, created_at)
                values ($1, $2, $3)
                on conflict (access_key_id, bundle_id) do nothing
                "#,
            )
            .bind(&access_key_id)
            .bind(bundle_id)
            .bind(now)
            .execute(&mut *tx)
            .await
            .map_err(map_db_error)?;
        }
    }

    if let Some(quota) = quota {
        super::balance_store::set_key_quota(&mut tx, &access_key_id, quota).await?;
    }
    tx.commit().await.map_err(map_db_error)?;
    clear_access_projection_cache(redis_pool).await?;
    let row = list_access_key_rows(pool, &access_key_id).await?;
    to_access_key_view(row, api_key_secret)
}

pub async fn read_access_key_secret(
    pool: &PgPool,
    id: &str,
    api_key_secret: Option<&str>,
) -> Result<String, GatewayError> {
    let row = list_access_key_rows(pool, id).await?;
    if row.status != "active"
        || row.revoked_at.is_some()
        || row.expires_at.is_some_and(|expiry| expiry <= now_utc())
    {
        return Err(GatewayError::unauthorized(
            "Access key is inactive or expired",
        ));
    }
    build_access_key_token(&row, api_key_secret)?.ok_or_else(|| {
        GatewayError::conflict("Key plaintext is unavailable")
            .with_code("access_key_secret_not_saved")
    })
}

pub async fn delete_access_key(
    pool: &PgPool,
    redis_pool: &RedisPool,
    access_key_id: &str,
) -> Result<DeleteAccessKeyResult, GatewayError> {
    let mut tx = pool.begin().await.map_err(map_db_error)?;
    let key_row = sqlx::query(
        r#"
        select id, display_name
        from gateway_access_keys
        where id = $1
        limit 1
        "#,
    )
    .bind(access_key_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(map_db_error)?
    .ok_or_else(|| GatewayError::not_found("access key 不存在"))?;

    sqlx::query("delete from gateway_access_keys where id = $1")
        .bind(access_key_id)
        .execute(&mut *tx)
        .await
        .map_err(map_db_error)?;

    bump_all_access_projection_versions(&mut tx, now_utc()).await?;
    tx.commit().await.map_err(map_db_error)?;
    clear_access_projection_cache(redis_pool).await?;

    Ok(DeleteAccessKeyResult {
        access_key_id: key_row.get::<String, _>("id"),
        display_name: key_row.get::<String, _>("display_name"),
    })
}

pub async fn set_access_key_enabled(
    pool: &PgPool,
    redis_pool: &RedisPool,
    access_key_id: &str,
    enabled: bool,
) -> Result<(), GatewayError> {
    let mut tx = pool.begin().await.map_err(map_db_error)?;
    // Conditional update serializes against revocation/rotation without reviving terminal keys.
    let result = sqlx::query("update gateway_access_keys set status = $2, updated_at = $3 where id = $1 and status in ('active', 'disabled') and revoked_at is null")
        .bind(access_key_id).bind(if enabled { "active" } else { "disabled" })
        .bind(now_utc()).execute(&mut *tx).await.map_err(map_db_error)?;
    if result.rows_affected() == 0 {
        return Err(GatewayError::conflict(
            "Key is missing or cannot be toggled",
        ));
    }
    bump_all_access_projection_versions(&mut tx, now_utc()).await?;
    tx.commit().await.map_err(map_db_error)?;
    clear_access_projection_cache(redis_pool).await?;
    Ok(())
}

pub async fn revoke_access_key(
    pool: &PgPool,
    redis_pool: &RedisPool,
    access_key_id: &str,
    reason: Option<&str>,
) -> Result<(), GatewayError> {
    let result = sqlx::query(
        r#"
        update gateway_access_keys
        set status = 'revoked', revoked_at = $2, revoke_reason = $3, updated_at = $2
        where id = $1
        "#,
    )
    .bind(access_key_id)
    .bind(now_utc())
    .bind(reason)
    .execute(pool)
    .await
    .map_err(map_db_error)?;
    if result.rows_affected() == 0 {
        return Err(GatewayError::not_found("access key 不存在"));
    }
    clear_access_projection_cache(redis_pool).await?;
    Ok(())
}
