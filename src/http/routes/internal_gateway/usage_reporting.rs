//! Project prompt-cache and credential-model usage reporting.

use super::project_access::ProjectPath;
use super::{assert_management_access, required_pg_pool};
use crate::db;
use crate::error::GatewayError;
use crate::http::extractors::OptionalBearerToken;
use crate::redis::keys;
use crate::redis::usage_tracking::dequeue_usage_reports;
use crate::state::AppState;
use crate::usage_aggregation::aggregate_usage_reports_by_user_credential_model;
use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::Json;
use redis::AsyncCommands;
use serde::Deserialize;
use serde_json::Value;
use std::sync::Arc;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectPromptCacheQuery {
    pub created_from: Option<String>,
    pub created_to: Option<String>,
    pub input_price_per_million: Option<f64>,
    pub bucket_size: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageAggregateFlushBody {
    pub batch_size: Option<usize>,
    pub bucket_seconds: Option<i64>,
}

pub async fn summarize_project_prompt_cache(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Path(path): Path<ProjectPath>,
    Query(query): Query<ProjectPromptCacheQuery>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let summary = db::summarize_prompt_cache(
        required_pg_pool(state.as_ref())?,
        &db::RequestAuditFilters {
            project_id: Some(path.project_id.trim().to_string()),
            created_from: query.created_from,
            created_to: query.created_to,
            limit: query.limit,
            ..db::RequestAuditFilters::default()
        },
        query.input_price_per_million,
    )
    .await?;
    Ok(Json(serde_json::json!({ "summary": summary })))
}

pub async fn get_project_prompt_cache_trend_report(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Path(path): Path<ProjectPath>,
    Query(query): Query<ProjectPromptCacheQuery>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let report = db::get_prompt_cache_trend_report(
        required_pg_pool(state.as_ref())?,
        &db::RequestAuditFilters {
            project_id: Some(path.project_id.trim().to_string()),
            created_from: query.created_from,
            created_to: query.created_to,
            limit: query.limit,
            ..db::RequestAuditFilters::default()
        },
        query.input_price_per_million,
        query.bucket_size.as_deref(),
    )
    .await?;
    Ok(Json(serde_json::json!({ "report": report })))
}

pub async fn list_provider_credential_model_states(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Query(query): Query<db::CredentialModelStateFilters>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let states = if let Some(local) = &state.local_runtime {
        local.list_model_states(query).await?
    } else {
        db::list_provider_credential_model_states(required_pg_pool(state.as_ref())?, query).await?
    };
    Ok(Json(serde_json::json!({ "states": states })))
}

pub async fn flush_usage_aggregates(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Json(body): Json<UsageAggregateFlushBody>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let batch_size = body
        .batch_size
        .unwrap_or(state.config.usage_report_batch_size)
        .clamp(1, 10_000);
    let bucket_seconds = body.bucket_seconds.unwrap_or(3600).max(60);
    let reports = dequeue_usage_reports(&state.redis_pool, batch_size)
        .await
        .map_err(|error| GatewayError::server_error(format!("dequeue usage reports: {error}")))?;
    let buckets = aggregate_usage_reports_by_user_credential_model(&reports, bucket_seconds);
    db::upsert_usage_aggregate_buckets(required_pg_pool(state.as_ref())?, &buckets).await?;
    Ok(Json(serde_json::json!({
        "dequeued": reports.len(),
        "bucketCount": buckets.len(),
        "buckets": buckets
    })))
}

pub async fn list_usage_aggregates(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Query(query): Query<db::UsageAggregateFilters>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let buckets =
        db::list_usage_aggregate_buckets(required_pg_pool(state.as_ref())?, query).await?;
    Ok(Json(serde_json::json!({ "buckets": buckets })))
}

pub async fn summarize_usage_aggregates(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let queue_depth = usage_queue_depth(state.as_ref()).await.unwrap_or(-1);
    let summary =
        db::summarize_usage_aggregates(required_pg_pool(state.as_ref())?, queue_depth).await?;
    Ok(Json(serde_json::json!({ "summary": summary })))
}

async fn usage_queue_depth(state: &AppState) -> Result<i64, GatewayError> {
    let mut conn =
        state.redis_pool.get().await.map_err(|error| {
            GatewayError::server_error(format!("get redis connection: {error}"))
        })?;
    conn.llen::<_, i64>(keys::usage_reports_key())
        .await
        .map_err(|error| GatewayError::server_error(format!("LLEN usage report queue: {error}")))
}
