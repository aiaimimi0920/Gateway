//! Operator usage aggregates and catalog database reads.

use super::*;

pub(super) async fn load_provider_usage_aggregates(
    pool: &PgPool,
    provider_account_ids: &[String],
) -> Result<HashMap<String, GatewayUsageAggregate>, GatewayError> {
    if provider_account_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let rows = sqlx::query_as::<_, ProviderUsageAggregateRow>(
        r#"
        select
          provider_account_id,
          count(*)::bigint as request_count,
          count(*) filter (where status = 'failed')::bigint as failure_count,
          count(*) filter (where created_at >= now() - interval '10 minutes')::bigint as recent_request_count_10m,
          count(*) filter (
            where status = 'failed'
              and created_at >= now() - interval '10 minutes'
          )::bigint as recent_failure_count_10m,
          coalesce(sum(greatest(coalesce(prompt_tokens, 0), 0)), 0)::bigint as input_tokens,
          coalesce(sum(greatest(coalesce(completion_tokens, 0), 0)), 0)::bigint as output_tokens,
          0::bigint as thinking_tokens,
          coalesce(
            sum(
              greatest(coalesce(cache_creation_input_tokens, 0), 0) +
              greatest(coalesce(cache_read_input_tokens, 0), 0)
            ),
            0
          )::bigint as cached_tokens,
          coalesce(sum(greatest(coalesce(prompt_tokens, 0), 0)), 0)::bigint as prompt_tokens,
          coalesce(sum(greatest(coalesce(completion_tokens, 0), 0)), 0)::bigint as completion_tokens,
          coalesce(
            sum(
              greatest(
                coalesce(total_tokens, coalesce(prompt_tokens, 0) + coalesce(completion_tokens, 0)),
                0
              )
            ),
            0
          )::bigint as total_tokens,
          max(created_at) as last_request_at
        from gateway_request_audits
        where provider_account_id = any($1)
        group by provider_account_id
        "#,
    )
    .bind(provider_account_ids)
    .fetch_all(pool)
    .await
    .map_err(map_db_error)?;

    Ok(rows
        .into_iter()
        .map(|row| {
            (
                row.provider_account_id,
                GatewayUsageAggregate {
                    request_count: row.request_count.max(0) as usize,
                    failure_count: row.failure_count.max(0) as usize,
                    recent_request_count_10m: row.recent_request_count_10m.max(0) as usize,
                    recent_failure_count_10m: row.recent_failure_count_10m.max(0) as usize,
                    input_tokens: row.input_tokens.max(0),
                    output_tokens: row.output_tokens.max(0),
                    thinking_tokens: row.thinking_tokens.max(0),
                    cached_tokens: row.cached_tokens.max(0),
                    prompt_tokens: row.prompt_tokens.max(0),
                    completion_tokens: row.completion_tokens.max(0),
                    total_tokens: row.total_tokens.max(0),
                    last_request_at: row.last_request_at.map(format_timestamp),
                },
            )
        })
        .collect())
}

pub(super) async fn load_catalog_metadata(
    pool: &PgPool,
) -> Result<GatewayCatalogMetadataView, GatewayError> {
    let provider_account_count = count_rows(pool, "gateway_provider_accounts").await?;
    let model_alias_count = count_rows(pool, "gateway_model_aliases").await?;
    let route_policy_count = count_rows(pool, "gateway_route_policies").await?;

    Ok(GatewayCatalogMetadataView {
        provider_account_count,
        model_alias_count,
        route_policy_count,
        fetched_provider_accounts: provider_account_count,
        fetched_model_aliases: model_alias_count,
        fetched_route_policies: route_policy_count,
    })
}

pub(super) async fn load_provider_supported_models(
    pool: &PgPool,
    provider_account_ids: &[String],
) -> Result<HashMap<String, Vec<String>>, GatewayError> {
    if provider_account_ids.is_empty() {
        return Ok(HashMap::new());
    }
    let rows = sqlx::query_as::<_, ProviderSupportedModelRow>(
        r#"
        select
          provider_account_id,
          coalesce(nullif(trim(upstream_model), ''), nullif(trim(model_code), '')) as model_name
        from gateway_provider_capability_catalog
        where enabled = true
          and provider_account_id = any($1)
        group by provider_account_id, coalesce(nullif(trim(upstream_model), ''), nullif(trim(model_code), ''))
        order by provider_account_id, coalesce(nullif(trim(upstream_model), ''), nullif(trim(model_code), ''))
        "#,
    )
    .bind(provider_account_ids)
    .fetch_all(pool)
    .await
    .map_err(map_db_error)?;

    let mut supported_models = HashMap::<String, Vec<String>>::new();
    for row in rows {
        if row.model_name.trim().is_empty() {
            continue;
        }
        supported_models
            .entry(row.provider_account_id)
            .or_default()
            .push(row.model_name);
    }
    Ok(supported_models)
}

pub(super) async fn load_provider_model_usage_aggregates(
    pool: &PgPool,
) -> Result<Vec<ProviderModelUsageAggregateRow>, GatewayError> {
    sqlx::query_as::<_, ProviderModelUsageAggregateRow>(
        r#"
        select
          source.provider_account_id,
          source.model_name,
          count(*)::bigint as request_count,
          coalesce(sum(greatest(coalesce(source.prompt_tokens, 0), 0)), 0)::bigint as input_tokens,
          coalesce(sum(greatest(coalesce(source.completion_tokens, 0), 0)), 0)::bigint as output_tokens,
          0::bigint as thinking_tokens,
          coalesce(
            sum(
              greatest(coalesce(source.cache_creation_input_tokens, 0), 0) +
              greatest(coalesce(source.cache_read_input_tokens, 0), 0)
            ),
            0
          )::bigint as cached_tokens,
          coalesce(sum(greatest(coalesce(source.prompt_tokens, 0), 0)), 0)::bigint as prompt_tokens,
          coalesce(sum(greatest(coalesce(source.completion_tokens, 0), 0)), 0)::bigint as completion_tokens,
          coalesce(
            sum(
              greatest(
                coalesce(source.total_tokens, coalesce(source.prompt_tokens, 0) + coalesce(source.completion_tokens, 0)),
                0
              )
            ),
            0
          )::bigint as total_tokens,
          max(source.created_at) as last_request_at
        from (
          select
            provider_account_id,
            coalesce(
              nullif(trim(resolved_model), ''),
              nullif(trim(requested_model), ''),
              nullif(trim(model_alias), ''),
              'unknown'
            ) as model_name,
            prompt_tokens,
            completion_tokens,
            cache_creation_input_tokens,
            cache_read_input_tokens,
            total_tokens,
            created_at
          from gateway_request_audits
          where provider_account_id is not null
        ) as source
        group by source.provider_account_id, source.model_name
        order by source.provider_account_id, source.model_name
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(map_db_error)
}

async fn count_rows(pool: &PgPool, table_name: &str) -> Result<usize, GatewayError> {
    let sql = format!("select count(*)::bigint as count from {table_name}");
    let count = sqlx::query_scalar::<_, i64>(&sql)
        .fetch_one(pool)
        .await
        .map_err(map_db_error)?;
    Ok(count.max(0) as usize)
}
