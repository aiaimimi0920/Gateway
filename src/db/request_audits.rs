use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::types::Json;
use sqlx::{FromRow, PgPool, Postgres, QueryBuilder, Row};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::error::GatewayError;

use super::{format_timestamp, map_db_error};

#[derive(Debug, Clone)]
pub struct CreateRequestAuditInput {
    pub project_id: String,
    pub api_key_id: Option<String>,
    pub user_credential_id: Option<String>,
    pub access_key_id: Option<String>,
    pub source_access_key_id: Option<String>,
    pub session_id: Option<String>,
    pub route_policy_id: Option<String>,
    pub provider_account_id: Option<String>,
    pub protocol_family: String,
    pub endpoint_kind: String,
    pub requested_model: Option<String>,
    pub resolved_model: Option<String>,
    pub model_alias: Option<String>,
    pub stream: bool,
    pub route_attempt_count: u32,
    pub response_id: String,
    pub previous_response_id: Option<String>,
    pub route_trace: Option<Value>,
}

#[derive(Debug, Clone)]
pub struct FinalizeRequestAuditInput {
    pub status: String,
    pub upstream_status: Option<u16>,
    pub duration_ms: u64,
    pub prompt_tokens: Option<u64>,
    pub completion_tokens: Option<u64>,
    pub total_tokens: Option<u64>,
    pub cache_creation_input_tokens: Option<u64>,
    pub cache_read_input_tokens: Option<u64>,
    pub client_has_cache_control: bool,
    pub auto_cache_applied: bool,
    pub error_summary: Option<String>,
    pub access_key_id: Option<String>,
    pub source_access_key_id: Option<String>,
    pub session_id: Option<String>,
    pub route_policy_id: Option<String>,
    pub provider_account_id: Option<String>,
    pub resolved_model: Option<String>,
    pub model_alias: Option<String>,
    pub route_attempt_count: u32,
    pub route_trace: Option<Value>,
    pub response_id: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestAuditFilters {
    pub project_id: Option<String>,
    pub route_policy_id: Option<String>,
    pub provider_account_id: Option<String>,
    pub session_id: Option<String>,
    pub api_key_id: Option<String>,
    pub user_credential_id: Option<String>,
    pub access_key_id: Option<String>,
    pub source_access_key_id: Option<String>,
    pub response_id: Option<String>,
    pub protocol_family: Option<String>,
    pub status: Option<String>,
    pub endpoint_kind: Option<String>,
    pub stream: Option<bool>,
    pub error_code: Option<String>,
    pub fallback_eligible: Option<bool>,
    pub artifact_available: Option<bool>,
    pub created_from: Option<String>,
    pub created_to: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayRequestAuditView {
    pub id: String,
    pub project_id: String,
    pub api_key_id: Option<String>,
    pub user_credential_id: Option<String>,
    pub access_key_id: Option<String>,
    pub source_access_key_id: Option<String>,
    pub session_id: Option<String>,
    pub route_policy_id: Option<String>,
    pub provider_account_id: Option<String>,
    pub protocol_family: String,
    pub endpoint_kind: String,
    pub requested_model: Option<String>,
    pub resolved_model: Option<String>,
    pub model_alias: Option<String>,
    pub stream: bool,
    pub status: String,
    pub upstream_status: Option<i32>,
    pub duration_ms: Option<i32>,
    pub prompt_tokens: Option<i32>,
    pub completion_tokens: Option<i32>,
    pub total_tokens: Option<i32>,
    pub cache_creation_input_tokens: Option<i32>,
    pub cache_read_input_tokens: Option<i32>,
    pub client_has_cache_control: bool,
    pub auto_cache_applied: bool,
    pub error_summary: Option<String>,
    pub route_trace: Option<Value>,
    pub analysis_profile: Option<Value>,
    pub request_artifact_object_key: Option<String>,
    pub response_artifact_object_key: Option<String>,
    pub response_id: String,
    pub previous_response_id: Option<String>,
    pub client_disconnected_at: Option<String>,
    pub created_at: String,
    pub completed_at: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayRequestAuditSummaryBucketView {
    pub value: String,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewaySummaryBucketKeyView {
    pub key: String,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayRequestAuditSummaryView {
    pub total_requests: usize,
    pub completed_count: usize,
    pub failed_count: usize,
    pub cancelled_count: usize,
    pub running_count: usize,
    pub fallback_eligible_failures: usize,
    pub fallback_exhausted_failures: usize,
    pub by_status: Vec<GatewayRequestAuditSummaryBucketView>,
    pub by_provider_account: Vec<GatewayRequestAuditSummaryBucketView>,
    pub by_endpoint_kind: Vec<GatewayRequestAuditSummaryBucketView>,
    pub by_error_code: Vec<GatewayRequestAuditSummaryBucketView>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisSampleView {
    pub request_audit_id: String,
    pub response_id: String,
    pub project_id: String,
    pub route_policy_id: Option<String>,
    pub session_id: Option<String>,
    pub provider_account_id: Option<String>,
    pub protocol_family: String,
    pub endpoint_kind: String,
    pub requested_model: Option<String>,
    pub resolved_model: Option<String>,
    pub status: String,
    pub stream: bool,
    pub created_at: String,
    pub completed_at: Option<String>,
    pub prompt_tokens: Option<i32>,
    pub completion_tokens: Option<i32>,
    pub total_tokens: Option<i32>,
    pub cache_creation_input_tokens: Option<i32>,
    pub cache_read_input_tokens: Option<i32>,
    pub analysis_profile: Option<Value>,
    pub request_artifact_object_key: Option<String>,
    pub response_artifact_object_key: Option<String>,
    pub route_trace: Option<Value>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisMetricDistributionView {
    pub avg: Option<f64>,
    pub p50: Option<f64>,
    pub p95: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisSummaryView {
    pub total_samples: usize,
    pub completed_samples: usize,
    pub failed_samples: usize,
    pub cancelled_samples: usize,
    pub stream_samples: usize,
    pub tool_request_samples: usize,
    pub tool_response_samples: usize,
    pub system_prompt_samples: usize,
    pub reasoning_samples: usize,
    pub metadata_samples: usize,
    pub explicit_session_samples: usize,
    pub previous_response_samples: usize,
    pub request_artifact_samples: usize,
    pub response_artifact_samples: usize,
    pub total_prompt_tokens: i64,
    pub total_completion_tokens: i64,
    pub total_tokens: i64,
    pub total_cache_creation_input_tokens: i64,
    pub total_cache_read_input_tokens: i64,
    pub request_text_chars: GatewayAnalysisMetricDistributionView,
    pub response_text_chars: GatewayAnalysisMetricDistributionView,
    pub first_token_latency_ms: GatewayAnalysisMetricDistributionView,
    pub stream_chunk_count: GatewayAnalysisMetricDistributionView,
    pub by_protocol_family: Vec<GatewayRequestAuditSummaryBucketView>,
    pub by_endpoint_kind: Vec<GatewayRequestAuditSummaryBucketView>,
    pub by_resolved_model: Vec<GatewayRequestAuditSummaryBucketView>,
    pub by_provider_account: Vec<GatewayRequestAuditSummaryBucketView>,
    pub by_status: Vec<GatewayRequestAuditSummaryBucketView>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayProviderRoutingAnalysisSummaryView {
    pub total_samples: usize,
    pub selected_provider_samples: usize,
    pub degraded_selected_provider_samples: usize,
    pub saturated_selected_provider_samples: usize,
    pub breaker_open_selected_provider_samples: usize,
    pub routing_score: GatewayAnalysisMetricDistributionView,
    pub health_weight: GatewayAnalysisMetricDistributionView,
    pub capacity_weight: GatewayAnalysisMetricDistributionView,
    pub by_selected_provider: Vec<GatewaySummaryBucketKeyView>,
    pub by_degradation_reason: Vec<GatewaySummaryBucketKeyView>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayProviderRoutingAnalysisFilterView {
    pub project_id: Option<String>,
    pub route_policy_id: Option<String>,
    pub provider_account_id: Option<String>,
    pub session_id: Option<String>,
    pub api_key_id: Option<String>,
    pub response_id: Option<String>,
    pub protocol_family: Option<String>,
    pub endpoint_kind: Option<String>,
    pub status: Option<String>,
    pub created_from: Option<String>,
    pub created_to: Option<String>,
    pub limit: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayProviderRoutingAnalysisAnomalyThresholdConfig {
    pub routing_score_warning_threshold: f64,
    pub routing_score_critical_threshold: f64,
    pub degraded_route_warning_threshold: f64,
    pub degraded_route_critical_threshold: f64,
    pub saturated_route_warning_threshold: f64,
    pub saturated_route_critical_threshold: f64,
    pub breaker_open_route_warning_threshold: f64,
    pub breaker_open_route_critical_threshold: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayProviderRoutingAnalysisAnomalyView {
    pub code: String,
    pub severity: String,
    pub message: String,
    pub latest_value: Option<f64>,
    pub previous_value: Option<f64>,
    pub delta_value: Option<f64>,
    pub delta_ratio: Option<f64>,
    pub threshold_value: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayProviderRoutingAnalysisAnomalyReportView {
    pub generated_at: String,
    pub filters: GatewayProviderRoutingAnalysisFilterView,
    pub profile_key: String,
    pub thresholds: GatewayProviderRoutingAnalysisAnomalyThresholdConfig,
    pub summary: GatewayProviderRoutingAnalysisSummaryView,
    pub anomalies: Vec<GatewayProviderRoutingAnalysisAnomalyView>,
    pub by_severity: Vec<GatewaySummaryBucketKeyView>,
    pub by_code: Vec<GatewaySummaryBucketKeyView>,
}

#[derive(Debug, Clone, Default)]
pub struct GatewayProviderRoutingAnalysisAnomalyOverrides {
    pub routing_score_warning_threshold: Option<f64>,
    pub routing_score_critical_threshold: Option<f64>,
    pub degraded_route_warning_threshold: Option<f64>,
    pub degraded_route_critical_threshold: Option<f64>,
    pub saturated_route_warning_threshold: Option<f64>,
    pub saturated_route_critical_threshold: Option<f64>,
    pub breaker_open_route_warning_threshold: Option<f64>,
    pub breaker_open_route_critical_threshold: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayPromptCacheMetricsView {
    pub hit_requests: usize,
    pub creation_requests: usize,
    pub client_marked_requests: usize,
    pub auto_applied_requests: usize,
    pub cache_creation_input_tokens: i64,
    pub cache_read_input_tokens: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayPromptCacheSummaryView {
    pub total_requests: usize,
    pub cache_hit_requests: usize,
    pub cache_creation_requests: usize,
    pub client_marked_requests: usize,
    pub auto_applied_requests: usize,
    pub cache_control_coverage_requests: usize,
    pub total_tokens_saved: i64,
    pub total_cache_creation_input_tokens: i64,
    pub estimated_cost_saved_usd: f64,
    pub cache_hit_rate: f64,
    pub cache_control_coverage_rate: f64,
    pub input_price_per_million: f64,
    pub cached_input_price_per_million: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayPromptCacheTrendPointView {
    pub bucket_start: String,
    pub total_requests: usize,
    pub cache_hit_requests: usize,
    pub cache_creation_requests: usize,
    pub client_marked_requests: usize,
    pub auto_applied_requests: usize,
    pub cache_control_coverage_requests: usize,
    pub total_tokens_saved: i64,
    pub total_cache_creation_input_tokens: i64,
    pub estimated_cost_saved_usd: f64,
    pub cache_hit_rate: f64,
    pub cache_control_coverage_rate: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayPromptCacheTrendReportView {
    pub bucket_size: String,
    pub summary: GatewayPromptCacheSummaryView,
    pub points: Vec<GatewayPromptCacheTrendPointView>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct GatewayRequestAnalysisProfilePayload {
    request_tool_count: Option<i64>,
    request_historical_tool_call_count: Option<i64>,
    has_system_prompt: Option<bool>,
    has_reasoning: Option<bool>,
    has_metadata: Option<bool>,
    has_explicit_session_key: Option<bool>,
    has_previous_response: Option<bool>,
    response_tool_call_count: Option<i64>,
    request_text_chars: Option<i64>,
    response_text_chars: Option<i64>,
    first_token_latency_ms: Option<i64>,
    stream_chunk_count: Option<i64>,
}

#[derive(Debug, Clone, FromRow)]
struct GatewayRequestAuditRow {
    id: String,
    project_id: String,
    api_key_id: Option<String>,
    user_credential_id: Option<String>,
    access_key_id: Option<String>,
    source_access_key_id: Option<String>,
    session_id: Option<String>,
    route_policy_id: Option<String>,
    provider_account_id: Option<String>,
    protocol_family: String,
    endpoint_kind: String,
    requested_model: Option<String>,
    resolved_model: Option<String>,
    model_alias: Option<String>,
    stream: bool,
    status: String,
    upstream_status: Option<i32>,
    duration_ms: Option<i32>,
    prompt_tokens: Option<i32>,
    completion_tokens: Option<i32>,
    total_tokens: Option<i32>,
    cache_creation_input_tokens: Option<i32>,
    cache_read_input_tokens: Option<i32>,
    client_has_cache_control: bool,
    auto_cache_applied: bool,
    error_summary: Option<String>,
    route_trace: Option<Json<Value>>,
    analysis_profile: Option<Json<Value>>,
    request_artifact_object_key: Option<String>,
    response_artifact_object_key: Option<String>,
    response_id: String,
    previous_response_id: Option<String>,
    client_disconnected_at: Option<OffsetDateTime>,
    created_at: OffsetDateTime,
    completed_at: Option<OffsetDateTime>,
    updated_at: OffsetDateTime,
}

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

pub async fn create_request_audit(
    pool: &PgPool,
    input: CreateRequestAuditInput,
) -> Result<String, GatewayError> {
    let request_audit_id = Uuid::new_v4().to_string();
    let timestamp = OffsetDateTime::now_utc();

    if input.api_key_id.is_none()
        && input.user_credential_id.is_none()
        && input.access_key_id.is_none()
    {
        return Err(GatewayError::bad_request(
            "request audit requires api_key_id, user_credential_id, or access_key_id",
        ));
    }

    sqlx::query(
        r#"
        insert into gateway_request_audits (
          id,
          project_id,
          api_key_id,
          user_credential_id,
          access_key_id,
          source_access_key_id,
          session_id,
          route_policy_id,
          provider_account_id,
          protocol_family,
          endpoint_kind,
          requested_model,
          resolved_model,
          model_alias,
          stream,
          route_attempt_count,
          status,
          upstream_status,
          duration_ms,
          prompt_tokens,
          completion_tokens,
          total_tokens,
          cache_creation_input_tokens,
          cache_read_input_tokens,
          error_summary,
          route_trace,
          analysis_profile,
          request_artifact_object_key,
          response_artifact_object_key,
          response_id,
          previous_response_id,
          client_disconnected_at,
          created_at,
          completed_at,
          updated_at
        ) values (
          $1, $2, $3, $4, $5, $6, $7, $8, $9, $10,
          $11, $12, $13, $14, $15, $16, 'running', null, null, null, null, null, null, null, null,
          $17, null, null, null, $18, $19, null, $20, null, $20
        )
        "#,
    )
    .bind(&request_audit_id)
    .bind(&input.project_id)
    .bind(input.api_key_id.as_deref())
    .bind(input.user_credential_id.as_deref())
    .bind(input.access_key_id.as_deref())
    .bind(input.source_access_key_id.as_deref())
    .bind(input.session_id.as_deref())
    .bind(input.route_policy_id.as_deref())
    .bind(input.provider_account_id.as_deref())
    .bind(&input.protocol_family)
    .bind(&input.endpoint_kind)
    .bind(input.requested_model.as_deref())
    .bind(input.resolved_model.as_deref())
    .bind(input.model_alias.as_deref())
    .bind(input.stream)
    .bind(i32::try_from(input.route_attempt_count.max(1)).unwrap_or(i32::MAX))
    .bind(input.route_trace.map(Json))
    .bind(&input.response_id)
    .bind(input.previous_response_id.as_deref())
    .bind(timestamp)
    .execute(pool)
    .await
    .map_err(map_db_error)?;

    Ok(request_audit_id)
}

pub async fn finalize_request_audit(
    pool: &PgPool,
    request_audit_id: &str,
    input: FinalizeRequestAuditInput,
) -> Result<(), GatewayError> {
    let timestamp = OffsetDateTime::now_utc();

    let result = sqlx::query(
        r#"
        update gateway_request_audits
        set
          status = $2,
          upstream_status = $3,
          duration_ms = $4,
          prompt_tokens = $5,
          completion_tokens = $6,
          total_tokens = $7,
          cache_creation_input_tokens = $8,
          cache_read_input_tokens = $9,
          client_has_cache_control = $10,
          auto_cache_applied = $11,
          error_summary = $12,
          route_trace = coalesce($13, route_trace),
          access_key_id = coalesce($14, access_key_id),
          source_access_key_id = coalesce($15, source_access_key_id),
          session_id = coalesce($16, session_id),
          route_policy_id = coalesce($17, route_policy_id),
          provider_account_id = coalesce($18, provider_account_id),
          resolved_model = coalesce($19, resolved_model),
          model_alias = coalesce($20, model_alias),
          route_attempt_count = $21,
          response_id = coalesce($22, response_id),
          completed_at = $23,
          updated_at = $23
        where id = $1
        "#,
    )
    .bind(request_audit_id)
    .bind(&input.status)
    .bind(input.upstream_status.map(i32::from))
    .bind(to_i32(input.duration_ms))
    .bind(input.prompt_tokens.map(to_i32))
    .bind(input.completion_tokens.map(to_i32))
    .bind(input.total_tokens.map(to_i32))
    .bind(input.cache_creation_input_tokens.map(to_i32))
    .bind(input.cache_read_input_tokens.map(to_i32))
    .bind(input.client_has_cache_control)
    .bind(input.auto_cache_applied)
    .bind(input.error_summary.as_deref())
    .bind(input.route_trace.map(Json))
    .bind(input.access_key_id.as_deref())
    .bind(input.source_access_key_id.as_deref())
    .bind(input.session_id.as_deref())
    .bind(input.route_policy_id.as_deref())
    .bind(input.provider_account_id.as_deref())
    .bind(input.resolved_model.as_deref())
    .bind(input.model_alias.as_deref())
    .bind(i32::try_from(input.route_attempt_count).unwrap_or(i32::MAX))
    .bind(input.response_id.as_deref())
    .bind(timestamp)
    .execute(pool)
    .await
    .map_err(map_db_error)?;

    if result.rows_affected() == 0 {
        return Err(GatewayError::not_found("AI gateway request audit 不存在"));
    }

    Ok(())
}

pub async fn update_request_audit_artifact_keys(
    pool: &PgPool,
    request_audit_id: &str,
    request_artifact_object_key: Option<&str>,
    response_artifact_object_key: Option<&str>,
) -> Result<(), GatewayError> {
    let timestamp = OffsetDateTime::now_utc();
    sqlx::query(
        r#"
        update gateway_request_audits
        set
          request_artifact_object_key = coalesce($2, request_artifact_object_key),
          response_artifact_object_key = coalesce($3, response_artifact_object_key),
          updated_at = $4
        where id = $1
        "#,
    )
    .bind(request_audit_id)
    .bind(request_artifact_object_key)
    .bind(response_artifact_object_key)
    .bind(timestamp)
    .execute(pool)
    .await
    .map_err(map_db_error)?;

    Ok(())
}

pub async fn list_request_audits(
    pool: &PgPool,
    filters: &RequestAuditFilters,
) -> Result<Vec<GatewayRequestAuditView>, GatewayError> {
    let (created_from, created_to) = parse_request_audit_created_range(filters)?;

    let limit = filters.limit.unwrap_or(200).clamp(1, 1000);
    let mut builder = QueryBuilder::<Postgres>::new(
        r#"
        select
          id, project_id, api_key_id, user_credential_id, access_key_id, source_access_key_id, session_id, route_policy_id, provider_account_id,
          protocol_family, endpoint_kind, requested_model, resolved_model, model_alias,
          stream, status, upstream_status, duration_ms, prompt_tokens, completion_tokens,
          total_tokens, cache_creation_input_tokens, cache_read_input_tokens, client_has_cache_control, auto_cache_applied,
          error_summary, route_trace, analysis_profile,
          request_artifact_object_key, response_artifact_object_key, response_id,
          previous_response_id, client_disconnected_at, created_at, completed_at, updated_at
        from gateway_request_audits
        where 1 = 1
        "#,
    );

    push_request_audit_filters(&mut builder, filters, created_from, created_to);

    builder
        .push(" order by created_at desc limit ")
        .push_bind(i64::try_from(limit).unwrap_or(1000));

    let rows = builder
        .build_query_as::<GatewayRequestAuditRow>()
        .fetch_all(pool)
        .await
        .map_err(map_db_error)?;

    Ok(rows.into_iter().map(to_request_audit_view).collect())
}

pub async fn summarize_request_audits(
    pool: &PgPool,
    filters: &RequestAuditFilters,
) -> Result<GatewayRequestAuditSummaryView, GatewayError> {
    let rows = list_request_audits(pool, filters).await?;

    let mut by_status = BTreeMap::new();
    let mut by_provider_account = BTreeMap::new();
    let mut by_endpoint_kind = BTreeMap::new();
    let mut by_error_code = BTreeMap::new();

    let mut completed_count = 0;
    let mut failed_count = 0;
    let mut cancelled_count = 0;
    let mut running_count = 0;
    let mut fallback_eligible_failures = 0;
    let mut fallback_exhausted_failures = 0;

    for row in &rows {
        accumulate_bucket(&mut by_status, Some(row.status.as_str()));
        accumulate_bucket(&mut by_provider_account, row.provider_account_id.as_deref());
        accumulate_bucket(&mut by_endpoint_kind, Some(row.endpoint_kind.as_str()));
        accumulate_bucket(
            &mut by_error_code,
            route_trace_error_code(row.route_trace.as_ref()),
        );

        match row.status.as_str() {
            "completed" => completed_count += 1,
            "failed" => {
                failed_count += 1;
                if route_trace_fallback_eligible(row.route_trace.as_ref()) {
                    fallback_eligible_failures += 1;
                } else {
                    fallback_exhausted_failures += 1;
                }
            }
            "cancelled" => cancelled_count += 1,
            "running" => running_count += 1,
            _ => {}
        }
    }

    Ok(GatewayRequestAuditSummaryView {
        total_requests: rows.len(),
        completed_count,
        failed_count,
        cancelled_count,
        running_count,
        fallback_eligible_failures,
        fallback_exhausted_failures,
        by_status: into_summary_buckets(by_status),
        by_provider_account: into_summary_buckets(by_provider_account),
        by_endpoint_kind: into_summary_buckets(by_endpoint_kind),
        by_error_code: into_summary_buckets(by_error_code),
    })
}

pub async fn get_request_audit(
    pool: &PgPool,
    request_audit_id: Option<&str>,
    response_id: Option<&str>,
) -> Result<Option<GatewayRequestAuditView>, GatewayError> {
    let request_audit_id = non_empty(request_audit_id);
    let response_id = non_empty(response_id);

    if request_audit_id.is_none() && response_id.is_none() {
        return Err(GatewayError::bad_request(
            "必须提供 requestAuditId 或 responseId",
        ));
    }

    let mut builder = QueryBuilder::<Postgres>::new(
        r#"
        select
          id, project_id, api_key_id, user_credential_id, access_key_id, source_access_key_id, session_id, route_policy_id, provider_account_id,
          protocol_family, endpoint_kind, requested_model, resolved_model, model_alias,
          stream, status, upstream_status, duration_ms, prompt_tokens, completion_tokens,
          total_tokens, cache_creation_input_tokens, cache_read_input_tokens, client_has_cache_control, auto_cache_applied,
          error_summary, route_trace, analysis_profile,
          request_artifact_object_key, response_artifact_object_key, response_id,
          previous_response_id, client_disconnected_at, created_at, completed_at, updated_at
        from gateway_request_audits
        where
        "#,
    );

    if let Some(value) = request_audit_id {
        builder.push(" id = ").push_bind(value);
    } else if let Some(value) = response_id {
        builder.push(" response_id = ").push_bind(value);
    }

    builder.push(" limit 1");

    let row = builder
        .build_query_as::<GatewayRequestAuditRow>()
        .fetch_optional(pool)
        .await
        .map_err(map_db_error)?;

    Ok(row.map(to_request_audit_view))
}

pub async fn list_analysis_samples(
    pool: &PgPool,
    filters: &RequestAuditFilters,
) -> Result<Vec<GatewayAnalysisSampleView>, GatewayError> {
    let rows = list_request_audits(pool, filters).await?;
    Ok(rows.into_iter().map(to_analysis_sample_view).collect())
}

pub async fn summarize_analysis(
    pool: &PgPool,
    filters: &RequestAuditFilters,
) -> Result<GatewayAnalysisSummaryView, GatewayError> {
    let rows = list_analysis_samples(pool, filters).await?;

    let mut by_protocol_family = BTreeMap::new();
    let mut by_endpoint_kind = BTreeMap::new();
    let mut by_resolved_model = BTreeMap::new();
    let mut by_provider_account = BTreeMap::new();
    let mut by_status = BTreeMap::new();

    let mut completed_samples = 0;
    let mut failed_samples = 0;
    let mut cancelled_samples = 0;
    let mut stream_samples = 0;
    let mut tool_request_samples = 0;
    let mut tool_response_samples = 0;
    let mut system_prompt_samples = 0;
    let mut reasoning_samples = 0;
    let mut metadata_samples = 0;
    let mut explicit_session_samples = 0;
    let mut previous_response_samples = 0;
    let mut request_artifact_samples = 0;
    let mut response_artifact_samples = 0;
    let mut total_prompt_tokens = 0_i64;
    let mut total_completion_tokens = 0_i64;
    let mut total_tokens = 0_i64;
    let mut total_cache_creation_input_tokens = 0_i64;
    let mut total_cache_read_input_tokens = 0_i64;

    let mut request_text_chars = Vec::new();
    let mut response_text_chars = Vec::new();
    let mut first_token_latency_ms = Vec::new();
    let mut stream_chunk_count = Vec::new();

    for row in &rows {
        accumulate_bucket(&mut by_protocol_family, Some(row.protocol_family.as_str()));
        accumulate_bucket(&mut by_endpoint_kind, Some(row.endpoint_kind.as_str()));
        accumulate_bucket(&mut by_resolved_model, row.resolved_model.as_deref());
        accumulate_bucket(&mut by_provider_account, row.provider_account_id.as_deref());
        accumulate_bucket(&mut by_status, Some(row.status.as_str()));

        match row.status.as_str() {
            "completed" => completed_samples += 1,
            "failed" => failed_samples += 1,
            "cancelled" => cancelled_samples += 1,
            _ => {}
        }

        if row.stream {
            stream_samples += 1;
        }

        let analysis_profile = parse_analysis_profile(row.analysis_profile.as_ref());
        if analysis_profile
            .as_ref()
            .is_some_and(|profile| profile.request_tool_count.unwrap_or(0) > 0)
            || analysis_profile
                .as_ref()
                .is_some_and(|profile| profile.request_historical_tool_call_count.unwrap_or(0) > 0)
        {
            tool_request_samples += 1;
        }
        if analysis_profile
            .as_ref()
            .is_some_and(|profile| profile.response_tool_call_count.unwrap_or(0) > 0)
        {
            tool_response_samples += 1;
        }
        if analysis_profile
            .as_ref()
            .is_some_and(|profile| profile.has_system_prompt.unwrap_or(false))
        {
            system_prompt_samples += 1;
        }
        if analysis_profile
            .as_ref()
            .is_some_and(|profile| profile.has_reasoning.unwrap_or(false))
        {
            reasoning_samples += 1;
        }
        if analysis_profile
            .as_ref()
            .is_some_and(|profile| profile.has_metadata.unwrap_or(false))
        {
            metadata_samples += 1;
        }
        if analysis_profile
            .as_ref()
            .is_some_and(|profile| profile.has_explicit_session_key.unwrap_or(false))
        {
            explicit_session_samples += 1;
        }
        if analysis_profile
            .as_ref()
            .is_some_and(|profile| profile.has_previous_response.unwrap_or(false))
        {
            previous_response_samples += 1;
        }
        if row.request_artifact_object_key.is_some() {
            request_artifact_samples += 1;
        }
        if row.response_artifact_object_key.is_some() {
            response_artifact_samples += 1;
        }

        total_prompt_tokens += i64::from(row.prompt_tokens.unwrap_or(0));
        total_completion_tokens += i64::from(row.completion_tokens.unwrap_or(0));
        total_tokens += i64::from(row.total_tokens.unwrap_or(0));
        total_cache_creation_input_tokens +=
            i64::from(row.cache_creation_input_tokens.unwrap_or(0));
        total_cache_read_input_tokens += i64::from(row.cache_read_input_tokens.unwrap_or(0));

        request_text_chars.push(
            analysis_profile
                .as_ref()
                .and_then(|profile| profile.request_text_chars),
        );
        response_text_chars.push(
            analysis_profile
                .as_ref()
                .and_then(|profile| profile.response_text_chars),
        );
        first_token_latency_ms.push(
            analysis_profile
                .as_ref()
                .and_then(|profile| profile.first_token_latency_ms),
        );
        stream_chunk_count.push(
            analysis_profile
                .as_ref()
                .and_then(|profile| profile.stream_chunk_count),
        );
    }

    Ok(GatewayAnalysisSummaryView {
        total_samples: rows.len(),
        completed_samples,
        failed_samples,
        cancelled_samples,
        stream_samples,
        tool_request_samples,
        tool_response_samples,
        system_prompt_samples,
        reasoning_samples,
        metadata_samples,
        explicit_session_samples,
        previous_response_samples,
        request_artifact_samples,
        response_artifact_samples,
        total_prompt_tokens,
        total_completion_tokens,
        total_tokens,
        total_cache_creation_input_tokens,
        total_cache_read_input_tokens,
        request_text_chars: build_distribution(&request_text_chars),
        response_text_chars: build_distribution(&response_text_chars),
        first_token_latency_ms: build_distribution(&first_token_latency_ms),
        stream_chunk_count: build_distribution(&stream_chunk_count),
        by_protocol_family: into_summary_buckets(by_protocol_family),
        by_endpoint_kind: into_summary_buckets(by_endpoint_kind),
        by_resolved_model: into_summary_buckets(by_resolved_model),
        by_provider_account: into_summary_buckets(by_provider_account),
        by_status: into_summary_buckets(by_status),
    })
}

pub async fn summarize_provider_routing_analysis(
    pool: &PgPool,
    filters: &RequestAuditFilters,
) -> Result<GatewayProviderRoutingAnalysisSummaryView, GatewayError> {
    let rows = list_analysis_samples(pool, filters).await?;
    Ok(build_provider_routing_summary(&rows))
}

pub async fn get_provider_routing_anomaly_report(
    pool: &PgPool,
    filters: &RequestAuditFilters,
    profile_key: Option<&str>,
    overrides: GatewayProviderRoutingAnalysisAnomalyOverrides,
) -> Result<GatewayProviderRoutingAnalysisAnomalyReportView, GatewayError> {
    let normalized_profile_key = normalize_provider_routing_profile_key(profile_key);
    let thresholds = build_provider_routing_anomaly_thresholds(&normalized_profile_key, overrides);
    let summary = summarize_provider_routing_analysis(pool, filters).await?;
    Ok(build_provider_routing_anomaly_report(
        filters,
        &normalized_profile_key,
        thresholds,
        summary,
    ))
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

fn to_request_audit_view(row: GatewayRequestAuditRow) -> GatewayRequestAuditView {
    GatewayRequestAuditView {
        id: row.id,
        project_id: row.project_id,
        api_key_id: row.api_key_id,
        user_credential_id: row.user_credential_id,
        access_key_id: row.access_key_id,
        source_access_key_id: row.source_access_key_id,
        session_id: row.session_id,
        route_policy_id: row.route_policy_id,
        provider_account_id: row.provider_account_id,
        protocol_family: row.protocol_family,
        endpoint_kind: row.endpoint_kind,
        requested_model: row.requested_model,
        resolved_model: row.resolved_model,
        model_alias: row.model_alias,
        stream: row.stream,
        status: row.status,
        upstream_status: row.upstream_status,
        duration_ms: row.duration_ms,
        prompt_tokens: row.prompt_tokens,
        completion_tokens: row.completion_tokens,
        total_tokens: row.total_tokens,
        cache_creation_input_tokens: row.cache_creation_input_tokens,
        cache_read_input_tokens: row.cache_read_input_tokens,
        client_has_cache_control: row.client_has_cache_control,
        auto_cache_applied: row.auto_cache_applied,
        error_summary: row.error_summary,
        route_trace: row.route_trace.map(|value| value.0),
        analysis_profile: row.analysis_profile.map(|value| value.0),
        request_artifact_object_key: row.request_artifact_object_key,
        response_artifact_object_key: row.response_artifact_object_key,
        response_id: row.response_id,
        previous_response_id: row.previous_response_id,
        client_disconnected_at: row.client_disconnected_at.map(format_timestamp),
        created_at: format_timestamp(row.created_at),
        completed_at: row.completed_at.map(format_timestamp),
        updated_at: format_timestamp(row.updated_at),
    }
}

fn to_analysis_sample_view(row: GatewayRequestAuditView) -> GatewayAnalysisSampleView {
    GatewayAnalysisSampleView {
        request_audit_id: row.id,
        response_id: row.response_id,
        project_id: row.project_id,
        route_policy_id: row.route_policy_id,
        session_id: row.session_id,
        provider_account_id: row.provider_account_id,
        protocol_family: row.protocol_family,
        endpoint_kind: row.endpoint_kind,
        requested_model: row.requested_model,
        resolved_model: row.resolved_model,
        status: row.status,
        stream: row.stream,
        created_at: row.created_at,
        completed_at: row.completed_at,
        prompt_tokens: row.prompt_tokens,
        completion_tokens: row.completion_tokens,
        total_tokens: row.total_tokens,
        cache_creation_input_tokens: row.cache_creation_input_tokens,
        cache_read_input_tokens: row.cache_read_input_tokens,
        analysis_profile: row.analysis_profile,
        request_artifact_object_key: row.request_artifact_object_key,
        response_artifact_object_key: row.response_artifact_object_key,
        route_trace: row.route_trace,
    }
}

fn build_provider_routing_summary(
    rows: &[GatewayAnalysisSampleView],
) -> GatewayProviderRoutingAnalysisSummaryView {
    let mut by_selected_provider = BTreeMap::new();
    let mut by_degradation_reason = BTreeMap::new();
    let mut selected_provider_samples = 0;
    let mut degraded_selected_provider_samples = 0;
    let mut saturated_selected_provider_samples = 0;
    let mut breaker_open_selected_provider_samples = 0;

    let mut routing_scores = Vec::new();
    let mut health_weights = Vec::new();
    let mut capacity_weights = Vec::new();

    for row in rows {
        let Some(selected) = route_trace_selected_candidate(row.route_trace.as_ref()) else {
            continue;
        };

        selected_provider_samples += 1;
        accumulate_bucket(
            &mut by_selected_provider,
            selected.get("providerAccountId").and_then(Value::as_str),
        );

        let routing_score = route_trace_selected_candidate_number(selected, "routingScore");
        let health_weight = route_trace_selected_candidate_number(selected, "healthWeight");
        let capacity_weight = route_trace_selected_candidate_number(selected, "capacityWeight");

        routing_scores.push(routing_score);
        health_weights.push(health_weight);
        capacity_weights.push(capacity_weight);

        if selected
            .get("degraded")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            degraded_selected_provider_samples += 1;
        }
        if selected
            .get("breakerOpen")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            breaker_open_selected_provider_samples += 1;
        }
        if capacity_weight.unwrap_or(1.0) <= 0.0 {
            saturated_selected_provider_samples += 1;
        }
        if let Some(reasons) = selected.get("degradationReasons").and_then(Value::as_array) {
            for reason in reasons.iter().filter_map(Value::as_str) {
                accumulate_bucket(&mut by_degradation_reason, Some(reason));
            }
        }
    }

    GatewayProviderRoutingAnalysisSummaryView {
        total_samples: rows.len(),
        selected_provider_samples,
        degraded_selected_provider_samples,
        saturated_selected_provider_samples,
        breaker_open_selected_provider_samples,
        routing_score: build_ts_distribution(&routing_scores),
        health_weight: build_ts_distribution(&health_weights),
        capacity_weight: build_ts_distribution(&capacity_weights),
        by_selected_provider: into_keyed_summary_buckets(by_selected_provider),
        by_degradation_reason: into_keyed_summary_buckets(by_degradation_reason),
    }
}

fn build_provider_routing_anomaly_report(
    filters: &RequestAuditFilters,
    profile_key: &str,
    thresholds: GatewayProviderRoutingAnalysisAnomalyThresholdConfig,
    summary: GatewayProviderRoutingAnalysisSummaryView,
) -> GatewayProviderRoutingAnalysisAnomalyReportView {
    let mut anomalies = Vec::new();
    let mut by_severity = BTreeMap::new();
    let mut by_code = BTreeMap::new();

    let selected_samples = summary.selected_provider_samples.max(1) as f64;
    let degraded_rate = summary.degraded_selected_provider_samples as f64 / selected_samples;
    let saturated_rate = summary.saturated_selected_provider_samples as f64 / selected_samples;
    let breaker_open_rate =
        summary.breaker_open_selected_provider_samples as f64 / selected_samples;
    let routing_score_avg = summary.routing_score.avg;

    if let Some(routing_score_avg) = routing_score_avg {
        if routing_score_avg <= thresholds.routing_score_warning_threshold {
            push_provider_routing_anomaly(
                &mut anomalies,
                "provider_routing_score_drop",
                if routing_score_avg <= thresholds.routing_score_critical_threshold {
                    "critical"
                } else {
                    "warning"
                },
                "当前窗口内被选中 provider 的平均 routing score 明显偏低，说明选路正在持续踩到退化节点。",
                Some(routing_score_avg),
                Some(thresholds.routing_score_warning_threshold),
            );
        }
    }

    if degraded_rate >= thresholds.degraded_route_warning_threshold {
        push_provider_routing_anomaly(
            &mut anomalies,
            "degraded_provider_route_spike",
            if degraded_rate >= thresholds.degraded_route_critical_threshold {
                "critical"
            } else {
                "warning"
            },
            "当前窗口内命中 degraded provider 的请求占比过高。",
            Some(round_metric(degraded_rate, 3)),
            Some(thresholds.degraded_route_warning_threshold),
        );
    }

    if saturated_rate >= thresholds.saturated_route_warning_threshold {
        push_provider_routing_anomaly(
            &mut anomalies,
            "saturated_provider_route_spike",
            if saturated_rate >= thresholds.saturated_route_critical_threshold {
                "critical"
            } else {
                "warning"
            },
            "当前窗口内命中并发饱和 provider 的请求占比过高。",
            Some(round_metric(saturated_rate, 3)),
            Some(thresholds.saturated_route_warning_threshold),
        );
    }

    if breaker_open_rate >= thresholds.breaker_open_route_warning_threshold {
        push_provider_routing_anomaly(
            &mut anomalies,
            "breaker_open_provider_route_detected",
            if breaker_open_rate >= thresholds.breaker_open_route_critical_threshold {
                "critical"
            } else {
                "warning"
            },
            "当前窗口内仍有请求命中 breaker-open provider，说明 routing 健康退避仍存在裂口。",
            Some(round_metric(breaker_open_rate, 3)),
            Some(thresholds.breaker_open_route_warning_threshold),
        );
    }

    for anomaly in &anomalies {
        accumulate_bucket(&mut by_severity, Some(anomaly.severity.as_str()));
        accumulate_bucket(&mut by_code, Some(anomaly.code.as_str()));
    }

    GatewayProviderRoutingAnalysisAnomalyReportView {
        generated_at: format_timestamp(OffsetDateTime::now_utc()),
        filters: to_provider_routing_filter_view(filters),
        profile_key: profile_key.to_string(),
        thresholds,
        summary,
        anomalies,
        by_severity: into_keyed_summary_buckets(by_severity),
        by_code: into_keyed_summary_buckets(by_code),
    }
}

fn push_request_audit_filters(
    builder: &mut QueryBuilder<Postgres>,
    filters: &RequestAuditFilters,
    created_from: Option<OffsetDateTime>,
    created_to: Option<OffsetDateTime>,
) {
    if let Some(value) = non_empty(filters.project_id.as_deref()) {
        builder
            .push(" and project_id = ")
            .push_bind(value.to_string());
    }
    if let Some(value) = non_empty(filters.route_policy_id.as_deref()) {
        builder
            .push(" and route_policy_id = ")
            .push_bind(value.to_string());
    }
    if let Some(value) = non_empty(filters.provider_account_id.as_deref()) {
        builder
            .push(" and provider_account_id = ")
            .push_bind(value.to_string());
    }
    if let Some(value) = non_empty(filters.session_id.as_deref()) {
        builder
            .push(" and session_id = ")
            .push_bind(value.to_string());
    }
    if let Some(value) = non_empty(filters.api_key_id.as_deref()) {
        builder
            .push(" and api_key_id = ")
            .push_bind(value.to_string());
    }
    if let Some(value) = non_empty(filters.user_credential_id.as_deref()) {
        builder
            .push(" and user_credential_id = ")
            .push_bind(value.to_string());
    }
    if let Some(value) = non_empty(filters.access_key_id.as_deref()) {
        builder
            .push(" and access_key_id = ")
            .push_bind(value.to_string());
    }
    if let Some(value) = non_empty(filters.source_access_key_id.as_deref()) {
        builder
            .push(" and source_access_key_id = ")
            .push_bind(value.to_string());
    }
    if let Some(value) = non_empty(filters.response_id.as_deref()) {
        builder
            .push(" and response_id = ")
            .push_bind(value.to_string());
    }
    if let Some(value) = non_empty(filters.protocol_family.as_deref()) {
        builder
            .push(" and protocol_family = ")
            .push_bind(value.to_string());
    }
    if let Some(value) = non_empty(filters.status.as_deref()) {
        builder.push(" and status = ").push_bind(value.to_string());
    }
    if let Some(value) = non_empty(filters.endpoint_kind.as_deref()) {
        builder
            .push(" and endpoint_kind = ")
            .push_bind(value.to_string());
    }
    if let Some(value) = non_empty(filters.error_code.as_deref()) {
        builder
            .push(" and route_trace ->> 'errorCode' = ")
            .push_bind(value.to_string());
    }
    if let Some(value) = filters.fallback_eligible {
        builder
            .push(" and coalesce((route_trace ->> 'fallbackEligible')::boolean, false) = ")
            .push_bind(value);
    }
    if let Some(value) = filters.artifact_available {
        if value {
            builder.push(
                " and (request_artifact_object_key is not null or response_artifact_object_key is not null)",
            );
        } else {
            builder.push(
                " and request_artifact_object_key is null and response_artifact_object_key is null",
            );
        }
    }
    if let Some(value) = filters.stream {
        builder.push(" and stream = ").push_bind(value);
    }
    if let Some(value) = created_from {
        builder.push(" and created_at >= ").push_bind(value);
    }
    if let Some(value) = created_to {
        builder.push(" and created_at <= ").push_bind(value);
    }
}

fn normalize_provider_routing_profile_key(profile_key: Option<&str>) -> String {
    match non_empty(profile_key) {
        Some("conservative") => "conservative".to_string(),
        Some("aggressive") => "aggressive".to_string(),
        _ => "balanced".to_string(),
    }
}

fn build_provider_routing_anomaly_thresholds(
    profile_key: &str,
    overrides: GatewayProviderRoutingAnalysisAnomalyOverrides,
) -> GatewayProviderRoutingAnalysisAnomalyThresholdConfig {
    let base = match profile_key {
        "conservative" => GatewayProviderRoutingAnalysisAnomalyThresholdConfig {
            routing_score_warning_threshold: 0.45,
            routing_score_critical_threshold: 0.25,
            degraded_route_warning_threshold: 0.4,
            degraded_route_critical_threshold: 0.65,
            saturated_route_warning_threshold: 0.15,
            saturated_route_critical_threshold: 0.3,
            breaker_open_route_warning_threshold: 0.05,
            breaker_open_route_critical_threshold: 0.15,
        },
        "aggressive" => GatewayProviderRoutingAnalysisAnomalyThresholdConfig {
            routing_score_warning_threshold: 0.65,
            routing_score_critical_threshold: 0.45,
            degraded_route_warning_threshold: 0.2,
            degraded_route_critical_threshold: 0.4,
            saturated_route_warning_threshold: 0.05,
            saturated_route_critical_threshold: 0.15,
            breaker_open_route_warning_threshold: 0.01,
            breaker_open_route_critical_threshold: 0.05,
        },
        _ => GatewayProviderRoutingAnalysisAnomalyThresholdConfig {
            routing_score_warning_threshold: 0.55,
            routing_score_critical_threshold: 0.35,
            degraded_route_warning_threshold: 0.3,
            degraded_route_critical_threshold: 0.55,
            saturated_route_warning_threshold: 0.1,
            saturated_route_critical_threshold: 0.2,
            breaker_open_route_warning_threshold: 0.02,
            breaker_open_route_critical_threshold: 0.08,
        },
    };

    GatewayProviderRoutingAnalysisAnomalyThresholdConfig {
        routing_score_warning_threshold: overrides
            .routing_score_warning_threshold
            .unwrap_or(base.routing_score_warning_threshold),
        routing_score_critical_threshold: overrides
            .routing_score_critical_threshold
            .unwrap_or(base.routing_score_critical_threshold),
        degraded_route_warning_threshold: overrides
            .degraded_route_warning_threshold
            .unwrap_or(base.degraded_route_warning_threshold),
        degraded_route_critical_threshold: overrides
            .degraded_route_critical_threshold
            .unwrap_or(base.degraded_route_critical_threshold),
        saturated_route_warning_threshold: overrides
            .saturated_route_warning_threshold
            .unwrap_or(base.saturated_route_warning_threshold),
        saturated_route_critical_threshold: overrides
            .saturated_route_critical_threshold
            .unwrap_or(base.saturated_route_critical_threshold),
        breaker_open_route_warning_threshold: overrides
            .breaker_open_route_warning_threshold
            .unwrap_or(base.breaker_open_route_warning_threshold),
        breaker_open_route_critical_threshold: overrides
            .breaker_open_route_critical_threshold
            .unwrap_or(base.breaker_open_route_critical_threshold),
    }
}

fn push_provider_routing_anomaly(
    anomalies: &mut Vec<GatewayProviderRoutingAnalysisAnomalyView>,
    code: &str,
    severity: &str,
    message: &str,
    latest_value: Option<f64>,
    threshold_value: Option<f64>,
) {
    anomalies.push(GatewayProviderRoutingAnalysisAnomalyView {
        code: code.to_string(),
        severity: severity.to_string(),
        message: message.to_string(),
        latest_value,
        previous_value: None,
        delta_value: None,
        delta_ratio: None,
        threshold_value,
    });
}

fn to_provider_routing_filter_view(
    filters: &RequestAuditFilters,
) -> GatewayProviderRoutingAnalysisFilterView {
    GatewayProviderRoutingAnalysisFilterView {
        project_id: filters.project_id.clone(),
        route_policy_id: filters.route_policy_id.clone(),
        provider_account_id: filters.provider_account_id.clone(),
        session_id: filters.session_id.clone(),
        api_key_id: filters.api_key_id.clone(),
        response_id: filters.response_id.clone(),
        protocol_family: filters.protocol_family.clone(),
        endpoint_kind: filters.endpoint_kind.clone(),
        status: filters.status.clone(),
        created_from: filters.created_from.clone(),
        created_to: filters.created_to.clone(),
        limit: filters.limit.unwrap_or(1000).clamp(1, 1000),
    }
}

fn route_trace_error_code(route_trace: Option<&Value>) -> Option<&str> {
    route_trace
        .and_then(|value| value.get("errorCode"))
        .and_then(Value::as_str)
}

fn route_trace_fallback_eligible(route_trace: Option<&Value>) -> bool {
    route_trace
        .and_then(|value| value.get("fallbackEligible"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

fn route_trace_selected_candidate(
    route_trace: Option<&Value>,
) -> Option<&serde_json::Map<String, Value>> {
    route_trace
        .and_then(|value| value.get("selectedCandidate"))
        .and_then(Value::as_object)
}

fn route_trace_selected_candidate_number(
    selected: &serde_json::Map<String, Value>,
    key: &str,
) -> Option<f64> {
    selected
        .get(key)
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite())
}

fn parse_analysis_profile(value: Option<&Value>) -> Option<GatewayRequestAnalysisProfilePayload> {
    value.and_then(|payload| serde_json::from_value(payload.clone()).ok())
}

fn accumulate_bucket(map: &mut BTreeMap<String, usize>, value: Option<&str>) {
    let key = value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("unknown")
        .to_string();
    *map.entry(key).or_default() += 1;
}

fn into_summary_buckets(map: BTreeMap<String, usize>) -> Vec<GatewayRequestAuditSummaryBucketView> {
    let mut buckets = map
        .into_iter()
        .map(|(value, count)| GatewayRequestAuditSummaryBucketView { value, count })
        .collect::<Vec<_>>();
    buckets.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.value.cmp(&right.value))
    });
    buckets
}

fn into_keyed_summary_buckets(map: BTreeMap<String, usize>) -> Vec<GatewaySummaryBucketKeyView> {
    let mut buckets = map
        .into_iter()
        .map(|(key, count)| GatewaySummaryBucketKeyView { key, count })
        .collect::<Vec<_>>();
    buckets.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.key.cmp(&right.key))
    });
    buckets
}

fn build_distribution(values: &[Option<i64>]) -> GatewayAnalysisMetricDistributionView {
    let mut normalized = values
        .iter()
        .flatten()
        .copied()
        .filter(|value| *value >= 0)
        .collect::<Vec<_>>();
    if normalized.is_empty() {
        return GatewayAnalysisMetricDistributionView {
            avg: None,
            p50: None,
            p95: None,
        };
    }

    normalized.sort_unstable();
    let sum = normalized.iter().sum::<i64>() as f64;
    GatewayAnalysisMetricDistributionView {
        avg: Some((sum / normalized.len() as f64 * 100.0).round() / 100.0),
        p50: percentile(&normalized, 0.5),
        p95: percentile(&normalized, 0.95),
    }
}

fn build_ts_distribution(values: &[Option<f64>]) -> GatewayAnalysisMetricDistributionView {
    let mut normalized = values
        .iter()
        .flatten()
        .copied()
        .filter(|value| value.is_finite())
        .collect::<Vec<_>>();
    if normalized.is_empty() {
        return GatewayAnalysisMetricDistributionView {
            avg: None,
            p50: None,
            p95: None,
        };
    }

    normalized.sort_by(|left, right| left.total_cmp(right));
    let average = normalized.iter().sum::<f64>() / normalized.len() as f64;

    GatewayAnalysisMetricDistributionView {
        avg: Some(round_metric(average, 3)),
        p50: percentile_by_rank(&normalized, 0.50),
        p95: percentile_by_rank(&normalized, 0.95),
    }
}

fn percentile(values: &[i64], ratio: f64) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let index = ((values.len() - 1) as f64 * ratio).floor() as usize;
    values.get(index).map(|value| *value as f64)
}

fn percentile_by_rank(values: &[f64], ratio: f64) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let index = (((ratio * values.len() as f64).ceil() as usize).max(1) - 1).min(values.len() - 1);
    values.get(index).copied()
}

fn round_metric(value: f64, precision: u32) -> f64 {
    let factor = 10_f64.powi(precision as i32);
    (value * factor).round() / factor
}

fn parse_request_audit_created_range(
    filters: &RequestAuditFilters,
) -> Result<(Option<OffsetDateTime>, Option<OffsetDateTime>), GatewayError> {
    let created_from = parse_optional_timestamp(filters.created_from.as_deref(), "createdFrom")?;
    let created_to = parse_optional_timestamp(filters.created_to.as_deref(), "createdTo")?;
    if let (Some(from), Some(to)) = (created_from, created_to) {
        if from > to {
            return Err(GatewayError::bad_request("createdFrom 不能晚于 createdTo"));
        }
    }
    Ok((created_from, created_to))
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

fn build_prompt_cache_summary_view(
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

fn parse_optional_timestamp(
    value: Option<&str>,
    field_name: &str,
) -> Result<Option<OffsetDateTime>, GatewayError> {
    let Some(value) = non_empty(value) else {
        return Ok(None);
    };

    OffsetDateTime::parse(value, &Rfc3339)
        .map(Some)
        .map_err(|_| GatewayError::bad_request(format!("{field_name} 必须是合法的 RFC3339 时间")))
}

fn non_empty(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn to_i32(value: u64) -> i32 {
    i32::try_from(value).unwrap_or(i32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_with_selected(route_trace: Value) -> GatewayAnalysisSampleView {
        GatewayAnalysisSampleView {
            request_audit_id: "req-1".to_string(),
            response_id: "resp-1".to_string(),
            project_id: "proj-1".to_string(),
            route_policy_id: Some("policy-1".to_string()),
            session_id: None,
            provider_account_id: Some("provider-a".to_string()),
            protocol_family: "openai".to_string(),
            endpoint_kind: "chat_completions".to_string(),
            requested_model: Some("gpt-4o".to_string()),
            resolved_model: Some("gpt-4o".to_string()),
            status: "completed".to_string(),
            stream: false,
            created_at: "2026-04-13T00:00:00Z".to_string(),
            completed_at: Some("2026-04-13T00:00:01Z".to_string()),
            prompt_tokens: Some(10),
            completion_tokens: Some(5),
            total_tokens: Some(15),
            cache_creation_input_tokens: None,
            cache_read_input_tokens: None,
            analysis_profile: None,
            request_artifact_object_key: None,
            response_artifact_object_key: None,
            route_trace: Some(route_trace),
        }
    }

    #[test]
    fn provider_routing_summary_counts_selected_candidates() {
        let rows = vec![
            sample_with_selected(json!({
                "selectedCandidate": {
                    "providerAccountId": "provider-a",
                    "routingScore": 0.61,
                    "healthWeight": 0.7,
                    "capacityWeight": 0.4,
                    "degraded": true,
                    "breakerOpen": false,
                    "degradationReasons": ["concurrency_pressure", "failure_count_elevated"]
                }
            })),
            sample_with_selected(json!({
                "selectedCandidate": {
                    "providerAccountId": "provider-b",
                    "routingScore": 0.92,
                    "healthWeight": 1.0,
                    "capacityWeight": 1.0,
                    "degraded": false,
                    "breakerOpen": true,
                    "degradationReasons": ["breaker_open"]
                }
            })),
            sample_with_selected(json!({
                "other": "ignored"
            })),
        ];

        let summary = build_provider_routing_summary(&rows);
        assert_eq!(summary.total_samples, 3);
        assert_eq!(summary.selected_provider_samples, 2);
        assert_eq!(summary.degraded_selected_provider_samples, 1);
        assert_eq!(summary.saturated_selected_provider_samples, 0);
        assert_eq!(summary.breaker_open_selected_provider_samples, 1);
        assert_eq!(summary.by_selected_provider[0].key, "provider-a");
        assert_eq!(summary.by_selected_provider[1].key, "provider-b");
        assert_eq!(summary.by_degradation_reason[0].count, 1);
    }

    #[test]
    fn provider_routing_distribution_matches_ts_rank_logic() {
        let values = vec![Some(0.1), Some(0.2), Some(0.3), Some(0.4)];
        let distribution = build_ts_distribution(&values);
        assert_eq!(distribution.avg, Some(0.25));
        assert_eq!(distribution.p50, Some(0.2));
        assert_eq!(distribution.p95, Some(0.4));
    }

    #[test]
    fn provider_routing_thresholds_match_balanced_defaults() {
        let thresholds = build_provider_routing_anomaly_thresholds(
            "balanced",
            GatewayProviderRoutingAnalysisAnomalyOverrides::default(),
        );
        assert_eq!(thresholds.routing_score_warning_threshold, 0.55);
        assert_eq!(thresholds.routing_score_critical_threshold, 0.35);
        assert_eq!(thresholds.breaker_open_route_warning_threshold, 0.02);
        assert_eq!(thresholds.breaker_open_route_critical_threshold, 0.08);
    }

    #[test]
    fn provider_routing_anomaly_report_flags_low_score_and_breaker_usage() {
        let filters = RequestAuditFilters {
            project_id: Some("proj-1".to_string()),
            limit: Some(1000),
            ..RequestAuditFilters::default()
        };
        let summary = GatewayProviderRoutingAnalysisSummaryView {
            total_samples: 4,
            selected_provider_samples: 4,
            degraded_selected_provider_samples: 2,
            saturated_selected_provider_samples: 1,
            breaker_open_selected_provider_samples: 1,
            routing_score: GatewayAnalysisMetricDistributionView {
                avg: Some(0.30),
                p50: Some(0.30),
                p95: Some(0.60),
            },
            health_weight: GatewayAnalysisMetricDistributionView {
                avg: Some(0.5),
                p50: Some(0.5),
                p95: Some(0.8),
            },
            capacity_weight: GatewayAnalysisMetricDistributionView {
                avg: Some(0.2),
                p50: Some(0.2),
                p95: Some(0.7),
            },
            by_selected_provider: vec![GatewaySummaryBucketKeyView {
                key: "provider-a".to_string(),
                count: 4,
            }],
            by_degradation_reason: vec![GatewaySummaryBucketKeyView {
                key: "breaker_open".to_string(),
                count: 1,
            }],
        };

        let report = build_provider_routing_anomaly_report(
            &filters,
            "balanced",
            build_provider_routing_anomaly_thresholds(
                "balanced",
                GatewayProviderRoutingAnalysisAnomalyOverrides::default(),
            ),
            summary,
        );

        assert_eq!(report.profile_key, "balanced");
        assert_eq!(report.anomalies.len(), 4);
        assert!(report
            .anomalies
            .iter()
            .any(|anomaly| anomaly.code == "provider_routing_score_drop"));
        assert!(report
            .anomalies
            .iter()
            .any(|anomaly| anomaly.code == "breaker_open_provider_route_detected"));
    }

    #[test]
    fn prompt_cache_summary_tracks_client_and_auto_applied_counts() {
        let summary = build_prompt_cache_summary_view(10, 4, 3, 2, 5, 1200, 800, 3.0);
        assert_eq!(summary.total_requests, 10);
        assert_eq!(summary.client_marked_requests, 2);
        assert_eq!(summary.auto_applied_requests, 5);
        assert_eq!(summary.cache_control_coverage_requests, 7);
        assert_eq!(summary.cache_hit_rate, 0.4);
        assert_eq!(summary.cache_control_coverage_rate, 0.7);
    }
}
