use super::projection_queries::{
    build_projection_rows_for_auto_route_key, build_projection_rows_for_normal_key,
};
use super::*;
use crate::redis::keys;
use redis::AsyncCommands;

const ACCESS_PROJECTION_CACHE_SCHEMA_VERSION: u16 = 2;

fn access_projection_cache_key(access_key_id: &str) -> String {
    keys::access_projection_key(access_key_id)
}

pub(super) async fn clear_access_projection_cache(
    redis_pool: &RedisPool,
) -> Result<(), GatewayError> {
    let mut conn = match redis_pool.get().await {
        Ok(conn) => conn,
        Err(error) => {
            tracing::warn!(error = %error, "access projection cache cleanup skipped");
            return Ok(());
        }
    };
    let mut cursor: u64 = 0;
    loop {
        let scan_result: Result<(u64, Vec<String>), _> = redis::cmd("SCAN")
            .arg(cursor)
            .arg("MATCH")
            .arg("gw:access:projection:*")
            .arg("COUNT")
            .arg(200)
            .query_async(&mut conn)
            .await;
        let (next_cursor, keys) = match scan_result {
            Ok(result) => result,
            Err(error) => {
                tracing::warn!(error = %error, "access projection cache scan failed");
                return Ok(());
            }
        };
        if !keys.is_empty() {
            let delete_result: Result<(), _> =
                redis::cmd("DEL").arg(keys).query_async(&mut conn).await;
            if let Err(error) = delete_result {
                tracing::warn!(error = %error, "access projection cache delete failed");
                return Ok(());
            }
        }
        cursor = next_cursor;
        if cursor == 0 {
            break;
        }
    }
    Ok(())
}

async fn load_model_alias_projection_rows(
    pool: &PgPool,
    resolved_project_id: &str,
) -> Result<Vec<ModelAliasProjectionRow>, GatewayError> {
    sqlx::query_as::<_, ModelAliasProjectionRow>(
        r#"
        select alias, provider_account_id, upstream_model
        from gateway_model_aliases
        where enabled = true
          and (project_id = $1 or project_id is null)
        order by alias asc, priority asc, weight desc, created_at asc
        "#,
    )
    .bind(resolved_project_id)
    .fetch_all(pool)
    .await
    .map_err(map_db_error)
}

pub(super) fn expand_projection_rows_with_aliases(
    rows: Vec<ProjectedPlatformAccessRow>,
    alias_rows: &[ModelAliasProjectionRow],
) -> Vec<ProjectedPlatformAccessRow> {
    let mut expanded = rows.clone();
    let mut seen = rows
        .iter()
        .map(|row| {
            (
                row.requesting_access_key_id.clone(),
                row.source_access_key_id.clone(),
                row.platform_access_id.clone(),
                row.model_code.clone(),
                row.endpoint_kind.clone(),
            )
        })
        .collect::<BTreeSet<_>>();

    for row in rows {
        let row_upstream = row
            .upstream_model
            .as_deref()
            .unwrap_or(row.model_code.as_str());
        for alias_row in alias_rows {
            if alias_row.provider_account_id != row.provider_account_id {
                continue;
            }
            let Some(alias_upstream) = alias_row.upstream_model.as_deref() else {
                continue;
            };
            if alias_upstream != row_upstream {
                continue;
            }
            let key = (
                row.requesting_access_key_id.clone(),
                row.source_access_key_id.clone(),
                row.platform_access_id.clone(),
                alias_row.alias.clone(),
                row.endpoint_kind.clone(),
            );
            if !seen.insert(key) {
                continue;
            }
            let mut alias_projection = row.clone();
            alias_projection.model_code = alias_row.alias.clone();
            expanded.push(alias_projection);
        }
    }

    expanded
}

pub(super) fn group_projection_rows_by_model(
    access_key: &AccessKeyAuthRecord,
    rows: Vec<ProjectedPlatformAccessRow>,
) -> CachedAccessProjection {
    let mut rows_by_model = BTreeMap::<String, Vec<ProjectedPlatformAccessRow>>::new();
    for row in rows {
        rows_by_model
            .entry(row.model_code.clone())
            .or_default()
            .push(row);
    }
    for rows in rows_by_model.values_mut() {
        rows.sort_by(|left, right| {
            right
                .routing_priority
                .cmp(&left.routing_priority)
                .then(right.operator_weight.cmp(&left.operator_weight))
                .then(
                    platform_tier_rank(&right.platform_tier)
                        .cmp(&platform_tier_rank(&left.platform_tier)),
                )
                .then(left.provider_account_id.cmp(&right.provider_account_id))
                .then(left.source_access_key_id.cmp(&right.source_access_key_id))
        });
    }
    CachedAccessProjection {
        schema_version: ACCESS_PROJECTION_CACHE_SCHEMA_VERSION,
        access_key_id: access_key.id.clone(),
        key_kind: access_key.key_kind.clone(),
        resolved_tenant_id: access_key.resolved_tenant_id.clone(),
        resolved_project_id: access_key.resolved_project_id.clone(),
        projection_version: access_key.projection_version.clone(),
        rows_by_model,
    }
}

async fn rebuild_access_projection(
    pool: &PgPool,
    redis_pool: &RedisPool,
    access_key: &AccessKeyAuthRecord,
) -> Result<CachedAccessProjection, GatewayError> {
    let rows = if access_key.key_kind == "auto_route" {
        build_projection_rows_for_auto_route_key(pool, &access_key.id).await?
    } else {
        build_projection_rows_for_normal_key(pool, &access_key.id).await?
    };
    let alias_rows =
        load_model_alias_projection_rows(pool, &access_key.resolved_project_id).await?;
    let rows = expand_projection_rows_with_aliases(rows, &alias_rows);
    let projection = group_projection_rows_by_model(access_key, rows);
    let payload = serde_json::to_string(&projection).map_err(|error| {
        GatewayError::server_error(format!("serialize access projection: {error}"))
    })?;
    let mut conn = redis_pool
        .get()
        .await
        .map_err(|error| GatewayError::server_error(format!("get redis connection: {error}")))?;
    let _: () = conn
        .set(access_projection_cache_key(&access_key.id), payload)
        .await
        .map_err(|error| {
            GatewayError::server_error(format!("write access projection cache: {error}"))
        })?;
    Ok(projection)
}

async fn get_cached_access_projection(
    redis_pool: &RedisPool,
    access_key: &AccessKeyAuthRecord,
) -> Result<Option<CachedAccessProjection>, GatewayError> {
    let mut conn = redis_pool
        .get()
        .await
        .map_err(|error| GatewayError::server_error(format!("get redis connection: {error}")))?;
    let raw: Option<String> = conn
        .get(access_projection_cache_key(&access_key.id))
        .await
        .map_err(|error| {
            GatewayError::server_error(format!("read access projection cache: {error}"))
        })?;
    let Some(raw) = raw else {
        return Ok(None);
    };
    let cached = match serde_json::from_str::<CachedAccessProjection>(&raw) {
        Ok(value) => value,
        Err(error) => {
            tracing::warn!(
                access_key_id = %access_key.id,
                error = %error,
                "discarding malformed access projection cache"
            );
            return Ok(None);
        }
    };
    Ok(cache_projection_is_compatible(&cached, access_key).then_some(cached))
}

pub(super) fn cache_projection_is_compatible(
    cached: &CachedAccessProjection,
    access_key: &AccessKeyAuthRecord,
) -> bool {
    cached.schema_version == ACCESS_PROJECTION_CACHE_SCHEMA_VERSION
        && cached.access_key_id == access_key.id
        && cached.key_kind == access_key.key_kind
        && cached.resolved_tenant_id == access_key.resolved_tenant_id
        && cached.resolved_project_id == access_key.resolved_project_id
        && cached.projection_version == access_key.projection_version
}

pub(super) async fn get_or_rebuild_access_projection(
    pool: &PgPool,
    redis_pool: &RedisPool,
    access_key: &AccessKeyAuthRecord,
) -> Result<CachedAccessProjection, GatewayError> {
    if let Some(projection) = get_cached_access_projection(redis_pool, access_key).await? {
        return Ok(projection);
    }
    rebuild_access_projection(pool, redis_pool, access_key).await
}

pub(super) fn collect_model_catalog_ids_from_access_projection(
    projection: &CachedAccessProjection,
) -> Vec<String> {
    let shadowed_upstream_models = projection
        .rows_by_model
        .values()
        .flat_map(|rows| rows.iter())
        .filter_map(|row| {
            let upstream_model = row
                .upstream_model
                .as_deref()
                .unwrap_or(row.model_code.as_str());
            (row.model_code != upstream_model)
                .then_some((row.provider_account_id.clone(), upstream_model.to_string()))
        })
        .collect::<BTreeSet<_>>();

    projection
        .rows_by_model
        .iter()
        .filter_map(|(model_id, rows)| {
            rows.iter()
                .any(|row| {
                    let upstream_model = row
                        .upstream_model
                        .as_deref()
                        .unwrap_or(row.model_code.as_str());
                    row.model_code != upstream_model
                        || !shadowed_upstream_models.contains(&(
                            row.provider_account_id.clone(),
                            upstream_model.to_string(),
                        ))
                })
                .then_some(model_id.clone())
        })
        .collect()
}

pub async fn list_models_for_access_key(
    pool: &PgPool,
    redis_pool: &RedisPool,
    access_key_id: &str,
) -> Result<Vec<ModelInfo>, GatewayError> {
    let access_key = find_access_key_auth_by_id(pool, access_key_id)
        .await?
        .ok_or_else(|| GatewayError::unauthorized("Access key not found"))?;
    validate_access_key_auth(&access_key)?;
    let projection = get_or_rebuild_access_projection(pool, redis_pool, &access_key).await?;
    let created = OffsetDateTime::now_utc().unix_timestamp();
    Ok(
        collect_model_catalog_ids_from_access_projection(&projection)
            .into_iter()
            .map(|id| ModelInfo {
                id,
                object: "model".to_string(),
                created,
                owned_by: "neuro-gateway".to_string(),
            })
            .collect(),
    )
}
