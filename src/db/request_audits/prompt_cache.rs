use super::*;

#[derive(Debug, Clone, FromRow)]
struct GatewayPromptCacheTrendRow {
    bucket_start: OffsetDateTime,
    total_requests: i64,
    cache_hit_requests: i64,
    cache_creation_requests: i64,
    client_marked_requests: i64,
    auto_applied_requests: i64,
    total_tokens_saved: i64,
    total_cache_creation_input_tokens: i64,
}

pub async fn get_prompt_cache_metrics(
    pool: &PgPool,
) -> Result<GatewayPromptCacheMetricsView, GatewayError> {
    let row = sqlx::query(
        r#"
        select
          count(*) filter (where coalesce(cache_read_input_tokens, 0) > 0) as hit_requests,
          count(*) filter (where coalesce(cache_creation_input_tokens, 0) > 0) as creation_requests,
          count(*) filter (where client_has_cache_control) as client_marked_requests,
          count(*) filter (where auto_cache_applied) as auto_applied_requests,
          coalesce(sum(cache_creation_input_tokens), 0) as cache_creation_input_tokens,
          coalesce(sum(cache_read_input_tokens), 0) as cache_read_input_tokens
        from gateway_request_audits
        "#,
    )
    .fetch_one(pool)
    .await
    .map_err(map_db_error)?;

    Ok(GatewayPromptCacheMetricsView {
        hit_requests: row.try_get::<i64, _>("hit_requests").unwrap_or(0).max(0) as usize,
        creation_requests: row
            .try_get::<i64, _>("creation_requests")
            .unwrap_or(0)
            .max(0) as usize,
        client_marked_requests: row
            .try_get::<i64, _>("client_marked_requests")
            .unwrap_or(0)
            .max(0) as usize,
        auto_applied_requests: row
            .try_get::<i64, _>("auto_applied_requests")
            .unwrap_or(0)
            .max(0) as usize,
        cache_creation_input_tokens: row
            .try_get::<i64, _>("cache_creation_input_tokens")
            .unwrap_or(0)
            .max(0),
        cache_read_input_tokens: row
            .try_get::<i64, _>("cache_read_input_tokens")
            .unwrap_or(0)
            .max(0),
    })
}

pub async fn summarize_prompt_cache(
    pool: &PgPool,
    filters: &RequestAuditFilters,
    input_price_per_million: Option<f64>,
) -> Result<GatewayPromptCacheSummaryView, GatewayError> {
    let input_price_per_million = normalize_prompt_cache_input_price(input_price_per_million)?;
    let (created_from, created_to) = parse_request_audit_created_range(filters)?;
    let mut builder = QueryBuilder::<Postgres>::new(
        r#"
        select
          count(*) as total_requests,
          count(*) filter (where coalesce(cache_read_input_tokens, 0) > 0) as cache_hit_requests,
          count(*) filter (where coalesce(cache_creation_input_tokens, 0) > 0) as cache_creation_requests,
          count(*) filter (where client_has_cache_control) as client_marked_requests,
          count(*) filter (where auto_cache_applied) as auto_applied_requests,
          coalesce(sum(cache_read_input_tokens), 0) as total_tokens_saved,
          coalesce(sum(cache_creation_input_tokens), 0) as total_cache_creation_input_tokens
        from gateway_request_audits
        where 1 = 1
        "#,
    );
    push_request_audit_filters(&mut builder, filters, created_from, created_to);

    let row = builder
        .build()
        .fetch_one(pool)
        .await
        .map_err(map_db_error)?;

    Ok(build_prompt_cache_summary_view(
        row.try_get::<i64, _>("total_requests").unwrap_or(0),
        row.try_get::<i64, _>("cache_hit_requests").unwrap_or(0),
        row.try_get::<i64, _>("cache_creation_requests")
            .unwrap_or(0),
        row.try_get::<i64, _>("client_marked_requests").unwrap_or(0),
        row.try_get::<i64, _>("auto_applied_requests").unwrap_or(0),
        row.try_get::<i64, _>("total_tokens_saved").unwrap_or(0),
        row.try_get::<i64, _>("total_cache_creation_input_tokens")
            .unwrap_or(0),
        input_price_per_million,
    ))
}

pub async fn get_prompt_cache_trend_report(
    pool: &PgPool,
    filters: &RequestAuditFilters,
    input_price_per_million: Option<f64>,
    bucket_size: Option<&str>,
) -> Result<GatewayPromptCacheTrendReportView, GatewayError> {
    let input_price_per_million = normalize_prompt_cache_input_price(input_price_per_million)?;
    let bucket_size = normalize_prompt_cache_bucket_size(bucket_size)?;
    let (created_from, created_to) = parse_request_audit_created_range(filters)?;
    let mut builder = QueryBuilder::<Postgres>::new(format!(
        r#"
        select
          date_trunc('{bucket_size}', created_at) as bucket_start,
          count(*) as total_requests,
          count(*) filter (where coalesce(cache_read_input_tokens, 0) > 0) as cache_hit_requests,
          count(*) filter (where coalesce(cache_creation_input_tokens, 0) > 0) as cache_creation_requests,
          count(*) filter (where client_has_cache_control) as client_marked_requests,
          count(*) filter (where auto_cache_applied) as auto_applied_requests,
          coalesce(sum(cache_read_input_tokens), 0) as total_tokens_saved,
          coalesce(sum(cache_creation_input_tokens), 0) as total_cache_creation_input_tokens
        from gateway_request_audits
        where 1 = 1
        "#
    ));
    push_request_audit_filters(&mut builder, filters, created_from, created_to);
    builder.push(" group by 1 order by 1 asc");

    let rows = builder
        .build_query_as::<GatewayPromptCacheTrendRow>()
        .fetch_all(pool)
        .await
        .map_err(map_db_error)?;

    let mut total_requests = 0_i64;
    let mut cache_hit_requests = 0_i64;
    let mut cache_creation_requests = 0_i64;
    let mut client_marked_requests = 0_i64;
    let mut auto_applied_requests = 0_i64;
    let mut total_tokens_saved = 0_i64;
    let mut total_cache_creation_input_tokens = 0_i64;

    let points = rows
        .into_iter()
        .map(|row| {
            total_requests += row.total_requests.max(0);
            cache_hit_requests += row.cache_hit_requests.max(0);
            cache_creation_requests += row.cache_creation_requests.max(0);
            client_marked_requests += row.client_marked_requests.max(0);
            auto_applied_requests += row.auto_applied_requests.max(0);
            total_tokens_saved += row.total_tokens_saved.max(0);
            total_cache_creation_input_tokens += row.total_cache_creation_input_tokens.max(0);

            let total_requests_usize = row.total_requests.max(0) as usize;
            let cache_hit_requests_usize = row.cache_hit_requests.max(0) as usize;
            let cache_creation_requests_usize = row.cache_creation_requests.max(0) as usize;
            let client_marked_requests_usize = row.client_marked_requests.max(0) as usize;
            let auto_applied_requests_usize = row.auto_applied_requests.max(0) as usize;
            let cache_control_coverage_requests =
                client_marked_requests_usize.saturating_add(auto_applied_requests_usize);
            GatewayPromptCacheTrendPointView {
                bucket_start: format_timestamp(row.bucket_start),
                total_requests: total_requests_usize,
                cache_hit_requests: cache_hit_requests_usize,
                cache_creation_requests: cache_creation_requests_usize,
                client_marked_requests: client_marked_requests_usize,
                auto_applied_requests: auto_applied_requests_usize,
                cache_control_coverage_requests,
                total_tokens_saved: row.total_tokens_saved.max(0),
                total_cache_creation_input_tokens: row.total_cache_creation_input_tokens.max(0),
                estimated_cost_saved_usd: round_metric(
                    calculate_prompt_cache_cost_saved_usd(
                        row.total_tokens_saved.max(0),
                        input_price_per_million,
                    ),
                    6,
                ),
                cache_hit_rate: round_metric(
                    if total_requests_usize == 0 {
                        0.0
                    } else {
                        cache_hit_requests_usize as f64 / total_requests_usize as f64
                    },
                    6,
                ),
                cache_control_coverage_rate: round_metric(
                    if total_requests_usize == 0 {
                        0.0
                    } else {
                        cache_control_coverage_requests as f64 / total_requests_usize as f64
                    },
                    6,
                ),
            }
        })
        .collect::<Vec<_>>();

    Ok(GatewayPromptCacheTrendReportView {
        bucket_size,
        summary: build_prompt_cache_summary_view(
            total_requests,
            cache_hit_requests,
            cache_creation_requests,
            client_marked_requests,
            auto_applied_requests,
            total_tokens_saved,
            total_cache_creation_input_tokens,
            input_price_per_million,
        ),
        points,
    })
}

fn normalize_prompt_cache_input_price(value: Option<f64>) -> Result<f64, GatewayError> {
    let value = value.unwrap_or(15.0);
    if !value.is_finite() || value <= 0.0 {
        return Err(GatewayError::bad_request("inputPricePerMillion 必须是正数"));
    }
    Ok(value)
}

fn normalize_prompt_cache_bucket_size(value: Option<&str>) -> Result<String, GatewayError> {
    match non_empty(value) {
        Some(bucket) if bucket.eq_ignore_ascii_case("hour") => Ok("hour".to_string()),
        Some(bucket) if bucket.eq_ignore_ascii_case("day") => Ok("day".to_string()),
        Some(_) => Err(GatewayError::bad_request("bucketSize 仅支持 hour 或 day")),
        None => Ok("day".to_string()),
    }
}

fn calculate_prompt_cache_cost_saved_usd(tokens_saved: i64, input_price_per_million: f64) -> f64 {
    if tokens_saved <= 0 {
        return 0.0;
    }
    tokens_saved as f64 * input_price_per_million * 0.9 / 1_000_000.0
}

pub(super) fn build_prompt_cache_summary_view(
    total_requests: i64,
    cache_hit_requests: i64,
    cache_creation_requests: i64,
    client_marked_requests: i64,
    auto_applied_requests: i64,
    total_tokens_saved: i64,
    total_cache_creation_input_tokens: i64,
    input_price_per_million: f64,
) -> GatewayPromptCacheSummaryView {
    let total_requests = total_requests.max(0) as usize;
    let cache_hit_requests = cache_hit_requests.max(0) as usize;
    let cache_creation_requests = cache_creation_requests.max(0) as usize;
    let client_marked_requests = client_marked_requests.max(0) as usize;
    let auto_applied_requests = auto_applied_requests.max(0) as usize;
    let cache_control_coverage_requests =
        client_marked_requests.saturating_add(auto_applied_requests);
    let total_tokens_saved = total_tokens_saved.max(0);
    let total_cache_creation_input_tokens = total_cache_creation_input_tokens.max(0);

    GatewayPromptCacheSummaryView {
        total_requests,
        cache_hit_requests,
        cache_creation_requests,
        client_marked_requests,
        auto_applied_requests,
        cache_control_coverage_requests,
        total_tokens_saved,
        total_cache_creation_input_tokens,
        estimated_cost_saved_usd: round_metric(
            calculate_prompt_cache_cost_saved_usd(total_tokens_saved, input_price_per_million),
            6,
        ),
        cache_hit_rate: round_metric(
            if total_requests == 0 {
                0.0
            } else {
                cache_hit_requests as f64 / total_requests as f64
            },
            6,
        ),
        cache_control_coverage_rate: round_metric(
            if total_requests == 0 {
                0.0
            } else {
                cache_control_coverage_requests as f64 / total_requests as f64
            },
            6,
        ),
        input_price_per_million: round_metric(input_price_per_million, 6),
        cached_input_price_per_million: round_metric(input_price_per_million * 0.1, 6),
    }
}
