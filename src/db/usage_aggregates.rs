use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool, Postgres, QueryBuilder};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use crate::error::GatewayError;
use crate::usage_aggregation::UsageAggregateBucket;

use super::{format_timestamp, map_db_error};

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageAggregateFilters {
    pub project_id: Option<String>,
    pub user_id: Option<String>,
    pub provider: Option<String>,
    pub provider_credential_ref: Option<String>,
    pub model: Option<String>,
    pub created_from: Option<String>,
    pub created_to: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayUsageAggregateBucketView {
    pub bucket_start: String,
    pub bucket_granularity: String,
    pub project_id: String,
    pub user_id: String,
    pub provider: String,
    pub provider_credential_ref: String,
    pub model: String,
    pub request_count: i64,
    pub failure_count: i64,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub total_tokens: i64,
    pub cache_creation_input_tokens: i64,
    pub cache_read_input_tokens: i64,
    pub latency_ms_sum: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, FromRow)]
struct GatewayUsageAggregateBucketRow {
    bucket_start: OffsetDateTime,
    bucket_granularity: String,
    project_id: String,
    user_id: String,
    provider: String,
    provider_credential_ref: String,
    model: String,
    request_count: i64,
    failure_count: i64,
    prompt_tokens: i64,
    completion_tokens: i64,
    total_tokens: i64,
    cache_creation_input_tokens: i64,
    cache_read_input_tokens: i64,
    latency_ms_sum: i64,
    created_at: OffsetDateTime,
    updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayUsageAggregateSummaryView {
    pub queue_depth: i64,
    pub recent_request_count: i64,
    pub recent_failure_count: i64,
    pub recent_total_tokens: i64,
    pub archive_failure_count: i64,
    pub alerts: Vec<GatewayUsageAggregateAlertView>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayUsageAggregateAlertView {
    pub severity: String,
    pub code: String,
    pub message: String,
}

pub async fn upsert_usage_aggregate_buckets(
    pool: &PgPool,
    buckets: &[UsageAggregateBucket],
) -> Result<(), GatewayError> {
    for bucket in buckets {
        let bucket_start =
            OffsetDateTime::parse(&bucket.bucket_start, &Rfc3339).map_err(|error| {
                GatewayError::bad_request(format!("invalid usage bucket timestamp: {error}"))
            })?;
        sqlx::query(
            r#"
            insert into gateway_usage_aggregates (
              bucket_start,
              bucket_granularity,
              project_id,
              user_id,
              provider,
              provider_credential_ref,
              model,
              request_count,
              failure_count,
              prompt_tokens,
              completion_tokens,
              total_tokens,
              cache_creation_input_tokens,
              cache_read_input_tokens,
              latency_ms_sum,
              created_at,
              updated_at
            ) values (
              $1, $2, $3, $4, $5, $6, $7, $8, $9, $10,
              $11, $12, $13, $14, $15, now(), now()
            )
            on conflict (
              bucket_start,
              bucket_granularity,
              project_id,
              user_id,
              provider,
              provider_credential_ref,
              model
            ) do update set
              request_count = gateway_usage_aggregates.request_count + excluded.request_count,
              failure_count = gateway_usage_aggregates.failure_count + excluded.failure_count,
              prompt_tokens = gateway_usage_aggregates.prompt_tokens + excluded.prompt_tokens,
              completion_tokens = gateway_usage_aggregates.completion_tokens + excluded.completion_tokens,
              total_tokens = gateway_usage_aggregates.total_tokens + excluded.total_tokens,
              cache_creation_input_tokens = gateway_usage_aggregates.cache_creation_input_tokens + excluded.cache_creation_input_tokens,
              cache_read_input_tokens = gateway_usage_aggregates.cache_read_input_tokens + excluded.cache_read_input_tokens,
              latency_ms_sum = gateway_usage_aggregates.latency_ms_sum + excluded.latency_ms_sum,
              updated_at = now()
            "#,
        )
        .bind(bucket_start)
        .bind(&bucket.bucket_granularity)
        .bind(&bucket.project_id)
        .bind(&bucket.user_id)
        .bind(&bucket.provider)
        .bind(&bucket.provider_credential_ref)
        .bind(&bucket.model)
        .bind(i64::try_from(bucket.request_count).unwrap_or(i64::MAX))
        .bind(i64::try_from(bucket.failure_count).unwrap_or(i64::MAX))
        .bind(i64::try_from(bucket.prompt_tokens).unwrap_or(i64::MAX))
        .bind(i64::try_from(bucket.completion_tokens).unwrap_or(i64::MAX))
        .bind(i64::try_from(bucket.total_tokens).unwrap_or(i64::MAX))
        .bind(i64::try_from(bucket.cache_creation_input_tokens).unwrap_or(i64::MAX))
        .bind(i64::try_from(bucket.cache_read_input_tokens).unwrap_or(i64::MAX))
        .bind(i64::try_from(bucket.latency_ms_sum).unwrap_or(i64::MAX))
        .execute(pool)
        .await
        .map_err(map_db_error)?;
    }
    Ok(())
}

pub async fn list_usage_aggregate_buckets(
    pool: &PgPool,
    filters: UsageAggregateFilters,
) -> Result<Vec<GatewayUsageAggregateBucketView>, GatewayError> {
    let mut builder =
        QueryBuilder::<Postgres>::new("select * from gateway_usage_aggregates where 1 = 1");

    push_optional_text_filter(&mut builder, "project_id", filters.project_id.as_deref());
    push_optional_text_filter(&mut builder, "user_id", filters.user_id.as_deref());
    push_optional_text_filter(&mut builder, "provider", filters.provider.as_deref());
    push_optional_text_filter(
        &mut builder,
        "provider_credential_ref",
        filters.provider_credential_ref.as_deref(),
    );
    push_optional_text_filter(&mut builder, "model", filters.model.as_deref());
    push_optional_timestamp_filter(
        &mut builder,
        "bucket_start",
        ">=",
        filters.created_from.as_deref(),
    )?;
    push_optional_timestamp_filter(
        &mut builder,
        "bucket_start",
        "<=",
        filters.created_to.as_deref(),
    )?;

    builder.push(" order by bucket_start desc limit ");
    builder.push_bind(i64::try_from(filters.limit.unwrap_or(200).clamp(1, 1000)).unwrap_or(200));

    let rows = builder
        .build_query_as::<GatewayUsageAggregateBucketRow>()
        .fetch_all(pool)
        .await
        .map_err(map_db_error)?;

    Ok(rows.into_iter().map(to_view).collect())
}

pub async fn summarize_usage_aggregates(
    pool: &PgPool,
    queue_depth: i64,
) -> Result<GatewayUsageAggregateSummaryView, GatewayError> {
    #[derive(Debug, FromRow)]
    struct SummaryRow {
        request_count: Option<i64>,
        failure_count: Option<i64>,
        total_tokens: Option<i64>,
    }
    #[derive(Debug, FromRow)]
    struct ArchiveFailureRow {
        archive_failure_count: Option<i64>,
    }

    let usage = sqlx::query_as::<_, SummaryRow>(
        r#"
        select
          coalesce(sum(request_count), 0) as request_count,
          coalesce(sum(failure_count), 0) as failure_count,
          coalesce(sum(total_tokens), 0) as total_tokens
        from gateway_usage_aggregates
        where bucket_start >= now() - interval '24 hours'
        "#,
    )
    .fetch_one(pool)
    .await
    .map_err(map_db_error)?;

    let archive_failure = sqlx::query_as::<_, ArchiveFailureRow>(
        r#"
        select count(*)::bigint as archive_failure_count
        from gateway_conversation_archives
        where created_at >= now() - interval '24 hours'
          and (archive_error is not null or status in ('archive_failed', 'partial'))
        "#,
    )
    .fetch_one(pool)
    .await
    .map_err(map_db_error)?;

    let recent_request_count = usage.request_count.unwrap_or(0);
    let recent_failure_count = usage.failure_count.unwrap_or(0);
    let recent_total_tokens = usage.total_tokens.unwrap_or(0);
    let archive_failure_count = archive_failure.archive_failure_count.unwrap_or(0);
    let mut alerts = Vec::new();

    if queue_depth > 10_000 {
        alerts.push(GatewayUsageAggregateAlertView {
            severity: "warning".to_string(),
            code: "usage_queue_depth_high".to_string(),
            message: format!("usage report queue depth is {queue_depth}"),
        });
    }

    if recent_request_count > 0
        && (recent_failure_count as f64 / recent_request_count as f64) >= 0.2
    {
        alerts.push(GatewayUsageAggregateAlertView {
            severity: "warning".to_string(),
            code: "usage_failure_rate_high".to_string(),
            message: format!(
                "24h failure rate is {}/{}",
                recent_failure_count, recent_request_count
            ),
        });
    }

    if recent_total_tokens > 100_000_000 {
        alerts.push(GatewayUsageAggregateAlertView {
            severity: "warning".to_string(),
            code: "usage_token_growth_high".to_string(),
            message: format!("24h token usage is {recent_total_tokens}"),
        });
    }

    if archive_failure_count > 0 {
        alerts.push(GatewayUsageAggregateAlertView {
            severity: "warning".to_string(),
            code: "conversation_archive_object_storage_failures".to_string(),
            message: format!("24h conversation archive failures: {archive_failure_count}"),
        });
    }

    Ok(GatewayUsageAggregateSummaryView {
        queue_depth,
        recent_request_count,
        recent_failure_count,
        recent_total_tokens,
        archive_failure_count,
        alerts,
    })
}

fn push_optional_text_filter(
    builder: &mut QueryBuilder<'_, Postgres>,
    column_name: &'static str,
    value: Option<&str>,
) {
    let Some(value) = value.and_then(trim_nonempty) else {
        return;
    };
    builder.push(" and ");
    builder.push(column_name);
    builder.push(" = ");
    builder.push_bind(value.to_string());
}

fn push_optional_timestamp_filter(
    builder: &mut QueryBuilder<'_, Postgres>,
    column_name: &'static str,
    op: &'static str,
    value: Option<&str>,
) -> Result<(), GatewayError> {
    let Some(value) = value.and_then(trim_nonempty) else {
        return Ok(());
    };
    let parsed = OffsetDateTime::parse(value, &Rfc3339)
        .map_err(|error| GatewayError::bad_request(format!("invalid timestamp: {error}")))?;
    builder.push(" and ");
    builder.push(column_name);
    builder.push(" ");
    builder.push(op);
    builder.push(" ");
    builder.push_bind(parsed);
    Ok(())
}

fn trim_nonempty(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then_some(trimmed)
}

fn to_view(row: GatewayUsageAggregateBucketRow) -> GatewayUsageAggregateBucketView {
    GatewayUsageAggregateBucketView {
        bucket_start: format_timestamp(row.bucket_start),
        bucket_granularity: row.bucket_granularity,
        project_id: row.project_id,
        user_id: row.user_id,
        provider: row.provider,
        provider_credential_ref: row.provider_credential_ref,
        model: row.model,
        request_count: row.request_count,
        failure_count: row.failure_count,
        prompt_tokens: row.prompt_tokens,
        completion_tokens: row.completion_tokens,
        total_tokens: row.total_tokens,
        cache_creation_input_tokens: row.cache_creation_input_tokens,
        cache_read_input_tokens: row.cache_read_input_tokens,
        latency_ms_sum: row.latency_ms_sum,
        created_at: format_timestamp(row.created_at),
        updated_at: format_timestamp(row.updated_at),
    }
}
