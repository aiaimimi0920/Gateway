use super::bundle_compat::expand_bundle_platform_access_ids_for_openai_compatibility;
use super::projection::clear_access_projection_cache;
use super::views::{to_access_bundle_item_view, to_access_bundle_view};
use super::*;

pub async fn save_access_bundle(
    pool: &PgPool,
    redis_pool: &RedisPool,
    bundle_id: Option<&str>,
    input: UpsertAccessBundleInput,
) -> Result<GatewayAccessBundleView, GatewayError> {
    let mut tx = pool.begin().await.map_err(map_db_error)?;
    let now = now_utc();
    let row = if let Some(id) = bundle_id {
        sqlx::query_as::<_, AccessBundleRow>(
            r#"
            update gateway_access_bundles
            set
              project_id = $2,
              slug = $3,
              display_name = $4,
              billing_mode = $5,
              status = $6,
              description = $7,
              metadata = $8,
              updated_at = $9
            where id = $1
            returning id, project_id, slug, display_name, billing_mode, status, description, metadata, created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(input.project_id.as_deref())
        .bind(input.slug.trim())
        .bind(input.display_name.trim())
        .bind(normalize_balance_mode(&input.billing_mode))
        .bind(normalize_status(&input.status))
        .bind(input.description.as_deref())
        .bind(input.metadata.clone().map(Json))
        .bind(now)
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_db_error)?
        .ok_or_else(|| GatewayError::not_found("access bundle 不存在"))?
    } else {
        sqlx::query_as::<_, AccessBundleRow>(
            r#"
            insert into gateway_access_bundles (
              id, project_id, slug, display_name, billing_mode, status, description, metadata, created_at, updated_at
            ) values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $9)
            returning id, project_id, slug, display_name, billing_mode, status, description, metadata, created_at, updated_at
            "#,
        )
        .bind(Uuid::new_v4().to_string())
        .bind(input.project_id.as_deref())
        .bind(input.slug.trim())
        .bind(input.display_name.trim())
        .bind(normalize_balance_mode(&input.billing_mode))
        .bind(normalize_status(&input.status))
        .bind(input.description.as_deref())
        .bind(input.metadata.clone().map(Json))
        .bind(now)
        .fetch_one(&mut *tx)
        .await
        .map_err(map_db_error)?
    };
    bump_all_access_projection_versions(&mut tx, now).await?;
    tx.commit().await.map_err(map_db_error)?;
    clear_access_projection_cache(redis_pool).await?;
    Ok(to_access_bundle_view(row))
}

pub async fn delete_access_bundle(
    pool: &PgPool,
    redis_pool: &RedisPool,
    bundle_id: &str,
) -> Result<DeleteAccessBundleResult, GatewayError> {
    let mut tx = pool.begin().await.map_err(map_db_error)?;
    let bundle_row = sqlx::query(
        r#"
        select id, display_name
        from gateway_access_bundles
        where id = $1
        limit 1
        "#,
    )
    .bind(bundle_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(map_db_error)?
    .ok_or_else(|| GatewayError::not_found("access bundle 不存在"))?;

    let deleted_platform_key_count = sqlx::query(
        r#"
        with target_keys as (
          select distinct ak.id
          from gateway_access_key_bundle_bindings kb
          join gateway_access_keys ak on ak.id = kb.access_key_id
          where kb.bundle_id = $1
            and ak.owner_type = 'platform'
            and ak.owner_id = 'bundle-platform-key'
            and ak.key_kind = 'normal'
        )
        delete from gateway_access_keys ak
        using target_keys tk
        where ak.id = tk.id
        "#,
    )
    .bind(bundle_id)
    .execute(&mut *tx)
    .await
    .map_err(map_db_error)?
    .rows_affected() as i64;

    sqlx::query("delete from gateway_access_bundles where id = $1")
        .bind(bundle_id)
        .execute(&mut *tx)
        .await
        .map_err(map_db_error)?;

    bump_all_access_projection_versions(&mut tx, now_utc()).await?;
    tx.commit().await.map_err(map_db_error)?;
    clear_access_projection_cache(redis_pool).await?;

    Ok(DeleteAccessBundleResult {
        bundle_id: bundle_row.get::<String, _>("id"),
        display_name: bundle_row.get::<String, _>("display_name"),
        deleted_platform_key_count,
    })
}

pub async fn replace_access_bundle_items(
    pool: &PgPool,
    redis_pool: &RedisPool,
    bundle_id: &str,
    platform_access_ids: &[String],
) -> Result<Vec<GatewayAccessBundleItemView>, GatewayError> {
    let mut tx = pool.begin().await.map_err(map_db_error)?;
    let platform_access_ids =
        expand_bundle_platform_access_ids_for_openai_compatibility(&mut tx, platform_access_ids)
            .await?;
    sqlx::query("delete from gateway_access_bundle_items where bundle_id = $1")
        .bind(bundle_id)
        .execute(&mut *tx)
        .await
        .map_err(map_db_error)?;
    let now = now_utc();
    for platform_access_id in &platform_access_ids {
        sqlx::query(
            r#"
            insert into gateway_access_bundle_items (bundle_id, platform_access_id, created_at)
            values ($1, $2, $3)
            on conflict (bundle_id, platform_access_id) do nothing
            "#,
        )
        .bind(bundle_id)
        .bind(platform_access_id)
        .bind(now)
        .execute(&mut *tx)
        .await
        .map_err(map_db_error)?;
    }
    bump_all_access_projection_versions(&mut tx, now).await?;
    tx.commit().await.map_err(map_db_error)?;
    clear_access_projection_cache(redis_pool).await?;
    let rows = sqlx::query_as::<_, AccessBundleItemRow>(
        r#"
        select bundle_id, platform_access_id, created_at
        from gateway_access_bundle_items
        where bundle_id = $1
        order by platform_access_id asc
        "#,
    )
    .bind(bundle_id)
    .fetch_all(pool)
    .await
    .map_err(map_db_error)?;
    Ok(rows.into_iter().map(to_access_bundle_item_view).collect())
}

pub async fn ensure_default_access_bundle_for_project(
    pool: &PgPool,
    redis_pool: &RedisPool,
    project_id: &str,
    display_name: &str,
) -> Result<GatewayAccessBundleView, GatewayError> {
    let slug = format!("project-{}-default", project_id.trim());
    let existing = sqlx::query_as::<_, AccessBundleRow>(
        r#"
        select id, project_id, slug, display_name, billing_mode, status, description, metadata, created_at, updated_at
        from gateway_access_bundles
        where project_id = $1 and slug = $2
        limit 1
        "#,
    )
    .bind(project_id)
    .bind(&slug)
    .fetch_optional(pool)
    .await
    .map_err(map_db_error)?;
    if let Some(row) = existing {
        return Ok(to_access_bundle_view(row));
    }
    save_access_bundle(
        pool,
        redis_pool,
        None,
        UpsertAccessBundleInput {
            project_id: Some(project_id.to_string()),
            slug,
            display_name: display_name.to_string(),
            billing_mode: "time_pass".to_string(),
            status: "active".to_string(),
            description: Some("默认访问包".to_string()),
            metadata: None,
        },
    )
    .await
}
