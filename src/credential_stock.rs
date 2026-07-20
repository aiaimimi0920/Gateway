use std::sync::Arc;
use std::time::Duration;

use deadpool_redis::Pool as RedisPool;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use sqlx::{FromRow, PgPool, Postgres, QueryBuilder};
use time::OffsetDateTime;
use tracing::{debug, warn};

use crate::error::GatewayError;
use crate::provider_quota::{read_cached_runtime_quota_snapshot, GatewayProviderQuotaView};
use crate::state::AppState;

use crate::db::{format_timestamp, map_db_error};

const DEFAULT_SIGNAL_STREAM: &str = "gw:credential-stock:signals";

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StockSeverity {
    Healthy,
    Warning,
    Critical,
    Overstock,
}

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CountWatermark {
    pub min: Option<usize>,
    pub target: Option<usize>,
    pub max: Option<usize>,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CountStockEvaluation {
    pub usable_credential_count: usize,
    pub min_credential_count: Option<usize>,
    pub target_credential_count: Option<usize>,
    pub max_credential_count: Option<usize>,
    pub needs_replenishment: bool,
    pub severity: StockSeverity,
    pub deficit_to_min: usize,
    pub deficit_to_target: usize,
    pub suggested_credential_top_up_count: usize,
    pub excess_over_max: usize,
}

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenWindowWatermark {
    pub min_average_available_tokens: Option<i64>,
    pub target_average_available_tokens: Option<i64>,
    pub target_credential_count: Option<usize>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialStockStatusFilters {
    pub stock_class_key: Option<String>,
    pub service_provider_key: Option<String>,
    pub implementation_line_key: Option<String>,
    pub provider_surface_key: Option<String>,
    pub credential_material_kind: Option<String>,
    pub provider_account_id: Option<String>,
    pub needs_replenishment: Option<bool>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpsertCredentialStockPolicyInput {
    #[serde(default)]
    pub id: Option<String>,
    pub stock_class_key: String,
    pub display_name: String,
    pub service_provider_key: String,
    pub implementation_line_key: String,
    pub provider_surface_key: String,
    pub credential_material_kind: String,
    #[serde(default)]
    pub provider_account_id: Option<String>,
    #[serde(default)]
    pub provider_adapter: Option<String>,
    #[serde(default)]
    pub selector: Option<Value>,
    #[serde(default, rename = "providerSupplyMetricKind")]
    pub provider_supply_metric_kind: Option<String>,
    #[serde(default, rename = "metricKind")]
    pub legacy_metric_kind: Option<String>,
    #[serde(default)]
    pub token_window_key: Option<String>,
    #[serde(default)]
    pub token_window_seconds: Option<i64>,
    #[serde(default)]
    pub min_credential_count: Option<i32>,
    #[serde(default)]
    pub target_credential_count: Option<i32>,
    #[serde(default)]
    pub max_credential_count: Option<i32>,
    #[serde(default)]
    pub min_average_available_tokens: Option<i64>,
    #[serde(default)]
    pub target_average_available_tokens: Option<i64>,
    #[serde(default)]
    pub signal_enabled: Option<bool>,
    #[serde(default)]
    pub signal_stream: Option<String>,
    #[serde(default)]
    pub signal_cooldown_secs: Option<i64>,
    #[serde(default)]
    pub enabled: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialStockPolicyView {
    pub id: String,
    pub stock_class_key: String,
    pub display_name: String,
    pub service_provider_key: String,
    pub implementation_line_key: String,
    pub provider_surface_key: String,
    pub credential_material_kind: String,
    pub provider_account_id: Option<String>,
    pub provider_adapter: Option<String>,
    pub selector: Value,
    #[serde(rename = "providerSupplyMetricKind")]
    pub provider_supply_metric_kind: String,
    #[serde(rename = "metricKind")]
    pub metric_kind: String,
    pub token_window_key: Option<String>,
    pub token_window_seconds: Option<i64>,
    pub min_credential_count: Option<i32>,
    pub target_credential_count: Option<i32>,
    pub max_credential_count: Option<i32>,
    pub min_average_available_tokens: Option<i64>,
    pub target_average_available_tokens: Option<i64>,
    pub signal_enabled: bool,
    pub signal_stream: String,
    pub signal_cooldown_secs: i64,
    pub enabled: bool,
    pub last_signal_key: Option<String>,
    pub last_signal_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialStockStatusView {
    pub policy: CredentialStockPolicyView,
    pub provider_supply_metric_kind: String,
    #[serde(rename = "metricKind")]
    pub metric_kind: String,
    pub token_window_key: Option<String>,
    pub token_window_seconds: Option<i64>,
    pub usable_credential_count: usize,
    pub active_credential_count: usize,
    pub cooling_credential_count: usize,
    pub disabled_credential_count: usize,
    pub archived_credential_count: usize,
    pub unknown_status_credential_count: usize,
    pub count_evaluation: CountStockEvaluation,
    pub token_evaluation: Option<TokenWindowStockEvaluation>,
    pub needs_replenishment: bool,
    pub severity: StockSeverity,
    pub suggested_credential_top_up_count: usize,
    pub signal_key: String,
    pub signal_payload: Value,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialStockStatusReport {
    pub policies: Vec<CredentialStockStatusView>,
    pub summary: CredentialStockSummaryView,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialStockSummaryView {
    pub total_policy_count: usize,
    pub enabled_policy_count: usize,
    pub needs_replenishment_policy_count: usize,
    pub critical_policy_count: usize,
    pub warning_policy_count: usize,
    pub healthy_policy_count: usize,
    pub overstock_policy_count: usize,
    pub total_suggested_credential_top_up_count: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialStockSignalSweepResult {
    pub scanned_policy_count: usize,
    pub replenishment_policy_count: usize,
    pub emitted_signal_count: usize,
    pub suppressed_signal_count: usize,
    pub redis_error_count: usize,
    pub signals: Vec<CredentialStockSignalEventView>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialStockSignalEventView {
    pub id: String,
    pub policy_id: String,
    pub stock_class_key: String,
    pub signal_key: String,
    pub severity: StockSeverity,
    pub stream: String,
    pub payload: Value,
    pub published_at: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, FromRow)]
struct CredentialStockPolicyRow {
    id: String,
    stock_class_key: String,
    display_name: String,
    service_provider_key: String,
    implementation_line_key: String,
    provider_surface_key: String,
    credential_material_kind: String,
    provider_account_id: Option<String>,
    provider_adapter: Option<String>,
    selector: sqlx::types::Json<Value>,
    metric_kind: String,
    token_window_key: Option<String>,
    token_window_seconds: Option<i64>,
    min_credential_count: Option<i32>,
    target_credential_count: Option<i32>,
    max_credential_count: Option<i32>,
    min_average_available_tokens: Option<i64>,
    target_average_available_tokens: Option<i64>,
    signal_enabled: bool,
    signal_stream: String,
    signal_cooldown_secs: i64,
    enabled: bool,
    last_signal_key: Option<String>,
    last_signal_at: Option<OffsetDateTime>,
    created_at: OffsetDateTime,
    updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, FromRow)]
struct CredentialStockCredentialRow {
    provider_credential_id: String,
    status: String,
    archived_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone, FromRow)]
struct CredentialStockSignalEventRow {
    id: String,
    policy_id: String,
    stock_class_key: String,
    signal_key: String,
    severity: String,
    stream: String,
    payload: sqlx::types::Json<Value>,
    published_at: Option<OffsetDateTime>,
    created_at: OffsetDateTime,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenWindowStockEvaluation {
    pub known_token_credential_count: usize,
    pub unknown_token_credential_count: usize,
    pub average_available_tokens: Option<i64>,
    pub min_average_available_tokens: Option<i64>,
    pub target_average_available_tokens: Option<i64>,
    pub target_credential_count: Option<usize>,
    pub needs_replenishment: bool,
    pub severity: StockSeverity,
    pub deficit_to_min_average_tokens: i64,
    pub deficit_to_target_average_tokens: i64,
    pub suggested_credential_top_up_count: usize,
}

pub fn evaluate_credential_count(
    usable_credential_count: usize,
    watermark: CountWatermark,
) -> CountStockEvaluation {
    let deficit_to_min = watermark
        .min
        .map(|minimum| minimum.saturating_sub(usable_credential_count))
        .unwrap_or(0);
    let deficit_to_target = watermark
        .target
        .map(|target| target.saturating_sub(usable_credential_count))
        .unwrap_or(deficit_to_min);
    let excess_over_max = watermark
        .max
        .map(|maximum| usable_credential_count.saturating_sub(maximum))
        .unwrap_or(0);
    let needs_replenishment = deficit_to_min > 0 || deficit_to_target > 0;
    let severity = if deficit_to_min > 0 {
        StockSeverity::Critical
    } else if deficit_to_target > 0 {
        StockSeverity::Warning
    } else if excess_over_max > 0 {
        StockSeverity::Overstock
    } else {
        StockSeverity::Healthy
    };

    CountStockEvaluation {
        usable_credential_count,
        min_credential_count: watermark.min,
        target_credential_count: watermark.target,
        max_credential_count: watermark.max,
        needs_replenishment,
        severity,
        deficit_to_min,
        deficit_to_target,
        suggested_credential_top_up_count: deficit_to_target.max(deficit_to_min),
        excess_over_max,
    }
}

pub fn evaluate_token_window(
    available_tokens_by_credential: &[Option<i64>],
    watermark: TokenWindowWatermark,
) -> TokenWindowStockEvaluation {
    let mut known_sum = 0i64;
    let mut known_count = 0usize;
    let mut unknown_count = 0usize;
    for value in available_tokens_by_credential {
        match value {
            Some(tokens) => {
                known_sum = known_sum.saturating_add((*tokens).max(0));
                known_count += 1;
            }
            None => unknown_count += 1,
        }
    }
    let average_available_tokens = (known_count > 0).then(|| known_sum / known_count as i64);
    let deficit_to_min_average_tokens = average_available_tokens
        .zip(watermark.min_average_available_tokens)
        .map(|(average, minimum)| minimum.saturating_sub(average).max(0))
        .unwrap_or(0);
    let deficit_to_target_average_tokens = average_available_tokens
        .zip(watermark.target_average_available_tokens)
        .map(|(average, target)| target.saturating_sub(average).max(0))
        .unwrap_or(deficit_to_min_average_tokens);
    let credential_count_deficit = watermark
        .target_credential_count
        .map(|target| target.saturating_sub(available_tokens_by_credential.len()))
        .unwrap_or(0);
    let needs_replenishment = deficit_to_min_average_tokens > 0
        || deficit_to_target_average_tokens > 0
        || credential_count_deficit > 0;
    let severity = if known_count == 0
        && (watermark.min_average_available_tokens.is_some()
            || watermark.target_average_available_tokens.is_some())
    {
        StockSeverity::Warning
    } else if deficit_to_min_average_tokens > 0 {
        StockSeverity::Critical
    } else if deficit_to_target_average_tokens > 0 || credential_count_deficit > 0 {
        StockSeverity::Warning
    } else {
        StockSeverity::Healthy
    };

    TokenWindowStockEvaluation {
        known_token_credential_count: known_count,
        unknown_token_credential_count: unknown_count,
        average_available_tokens,
        min_average_available_tokens: watermark.min_average_available_tokens,
        target_average_available_tokens: watermark.target_average_available_tokens,
        target_credential_count: watermark.target_credential_count,
        needs_replenishment,
        severity,
        deficit_to_min_average_tokens,
        deficit_to_target_average_tokens,
        suggested_credential_top_up_count: credential_count_deficit.max(
            if deficit_to_target_average_tokens > 0 {
                1usize.saturating_add(unknown_count)
            } else {
                0
            },
        ),
    }
}

pub fn build_signal_key(
    stock_class_key: &str,
    metric_kind: &str,
    deficit_credential_count: Option<usize>,
    token_window_key: Option<&str>,
) -> String {
    let normalized = format!(
        "{}\u{1f}{}\u{1f}{}\u{1f}{}",
        stock_class_key.trim().to_ascii_lowercase(),
        metric_kind.trim().to_ascii_lowercase(),
        deficit_credential_count.unwrap_or(0),
        token_window_key.unwrap_or("").trim().to_ascii_lowercase()
    );
    let mut hasher = Sha256::new();
    hasher.update(normalized.as_bytes());
    format!("credential_stock_signal_{}", hex::encode(hasher.finalize()))
}

pub async fn start_credential_stock_monitor_task(state: Arc<AppState>) {
    if !state.config.credential_stock_monitor_enabled {
        debug!("credential stock monitor disabled");
        return;
    }
    if state.pg_pool.is_none() {
        debug!("credential stock monitor disabled because PostgreSQL is not configured");
        return;
    }
    let interval_secs = state.config.credential_stock_monitor_interval_secs.max(15);
    let mut interval = tokio::time::interval(Duration::from_secs(interval_secs));
    loop {
        interval.tick().await;
        if state.lifecycle.is_draining() {
            break;
        }
        match sweep_credential_stock_signals_once(state.as_ref()).await {
            Ok(result) => {
                debug!(
                    scanned = result.scanned_policy_count,
                    emitted = result.emitted_signal_count,
                    suppressed = result.suppressed_signal_count,
                    redis_errors = result.redis_error_count,
                    "credential stock signal sweep finished"
                );
            }
            Err(error) => {
                warn!(
                    error = %error.message,
                    "credential stock signal sweep failed"
                );
            }
        }
    }
}

pub async fn list_credential_stock_policies(
    pool: &PgPool,
    filters: &CredentialStockStatusFilters,
) -> Result<Vec<CredentialStockPolicyView>, GatewayError> {
    let rows = query_policy_rows(pool, filters, false).await?;
    Ok(rows.into_iter().map(policy_view_from_row).collect())
}

pub async fn upsert_credential_stock_policy(
    pool: &PgPool,
    input: UpsertCredentialStockPolicyInput,
) -> Result<CredentialStockPolicyView, GatewayError> {
    let stock_class_key = normalize_required_key(&input.stock_class_key, "stockClassKey", 160)?;
    let display_name = normalize_required_text(&input.display_name, "displayName", 200)?;
    let service_provider_key =
        normalize_required_key(&input.service_provider_key, "serviceProviderKey", 120)?;
    let implementation_line_key =
        normalize_required_key(&input.implementation_line_key, "implementationLineKey", 160)?;
    let provider_surface_key =
        normalize_required_key(&input.provider_surface_key, "providerSurfaceKey", 160)?;
    let credential_material_kind =
        normalize_credential_material_kind(&input.credential_material_kind)?;
    let metric_kind = resolve_provider_supply_metric_kind(
        input.provider_supply_metric_kind.as_deref(),
        input.legacy_metric_kind.as_deref(),
    )?;
    let id = input
        .id
        .as_deref()
        .and_then(trim_nonempty)
        .map(str::to_string)
        .unwrap_or_else(|| format!("credential_stock_policy_{}", uuid::Uuid::new_v4()));
    let selector = input.selector.unwrap_or_else(|| serde_json::json!({}));
    let signal_stream = input
        .signal_stream
        .as_deref()
        .and_then(trim_nonempty)
        .unwrap_or(DEFAULT_SIGNAL_STREAM)
        .to_string();
    let now = OffsetDateTime::now_utc();

    let row = sqlx::query_as::<_, CredentialStockPolicyRow>(
        r#"
        insert into gateway_credential_stock_policies (
          id,
          stock_class_key,
          display_name,
          service_provider_key,
          implementation_line_key,
          provider_surface_key,
          credential_material_kind,
          provider_account_id,
          provider_adapter,
          selector,
          metric_kind,
          token_window_key,
          token_window_seconds,
          min_credential_count,
          target_credential_count,
          max_credential_count,
          min_average_available_tokens,
          target_average_available_tokens,
          signal_enabled,
          signal_stream,
          signal_cooldown_secs,
          enabled,
          created_at,
          updated_at
        ) values (
          $1, $2, $3, $4, $5, $6, $7, $8, $9, $10,
          $11, $12, $13, $14, $15, $16, $17, $18,
          $19, $20, $21, $22, $23, $23
        )
        on conflict (stock_class_key) do update set
          display_name = excluded.display_name,
          service_provider_key = excluded.service_provider_key,
          implementation_line_key = excluded.implementation_line_key,
          provider_surface_key = excluded.provider_surface_key,
          credential_material_kind = excluded.credential_material_kind,
          provider_account_id = excluded.provider_account_id,
          provider_adapter = excluded.provider_adapter,
          selector = excluded.selector,
          metric_kind = excluded.metric_kind,
          token_window_key = excluded.token_window_key,
          token_window_seconds = excluded.token_window_seconds,
          min_credential_count = excluded.min_credential_count,
          target_credential_count = excluded.target_credential_count,
          max_credential_count = excluded.max_credential_count,
          min_average_available_tokens = excluded.min_average_available_tokens,
          target_average_available_tokens = excluded.target_average_available_tokens,
          signal_enabled = excluded.signal_enabled,
          signal_stream = excluded.signal_stream,
          signal_cooldown_secs = excluded.signal_cooldown_secs,
          enabled = excluded.enabled,
          updated_at = excluded.updated_at
        returning *
        "#,
    )
    .bind(id)
    .bind(stock_class_key)
    .bind(display_name)
    .bind(service_provider_key)
    .bind(implementation_line_key)
    .bind(provider_surface_key)
    .bind(credential_material_kind)
    .bind(normalize_optional_key(
        input.provider_account_id.as_deref(),
        "providerAccountId",
        160,
    )?)
    .bind(normalize_optional_key(
        input.provider_adapter.as_deref(),
        "providerAdapter",
        120,
    )?)
    .bind(sqlx::types::Json(selector))
    .bind(metric_kind)
    .bind(normalize_optional_key(
        input.token_window_key.as_deref(),
        "tokenWindowKey",
        120,
    )?)
    .bind(input.token_window_seconds.filter(|value| *value > 0))
    .bind(input.min_credential_count.filter(|value| *value >= 0))
    .bind(input.target_credential_count.filter(|value| *value >= 0))
    .bind(input.max_credential_count.filter(|value| *value >= 0))
    .bind(
        input
            .min_average_available_tokens
            .filter(|value| *value >= 0),
    )
    .bind(
        input
            .target_average_available_tokens
            .filter(|value| *value >= 0),
    )
    .bind(input.signal_enabled.unwrap_or(true))
    .bind(signal_stream)
    .bind(input.signal_cooldown_secs.unwrap_or(300).max(0))
    .bind(input.enabled.unwrap_or(true))
    .bind(now)
    .fetch_one(pool)
    .await
    .map_err(map_db_error)?;

    Ok(policy_view_from_row(row))
}

pub async fn get_credential_stock_status_report(
    pool: &PgPool,
    redis_pool: &RedisPool,
    filters: CredentialStockStatusFilters,
) -> Result<CredentialStockStatusReport, GatewayError> {
    let policies = query_policy_rows(pool, &filters, true).await?;
    let mut statuses = Vec::with_capacity(policies.len());
    for row in policies {
        let status = evaluate_policy_row(pool, redis_pool, row).await?;
        if let Some(needs) = filters.needs_replenishment {
            if status.needs_replenishment != needs {
                continue;
            }
        }
        statuses.push(status);
    }
    Ok(CredentialStockStatusReport {
        summary: build_stock_summary(&statuses),
        policies: statuses,
    })
}

pub async fn sweep_credential_stock_signals_once(
    state: &AppState,
) -> Result<CredentialStockSignalSweepResult, GatewayError> {
    let pg_pool = state
        .pg_pool
        .as_ref()
        .ok_or_else(|| GatewayError::service_unavailable("PostgreSQL 尚未配置"))?;
    let report = get_credential_stock_status_report(
        pg_pool,
        &state.redis_pool,
        CredentialStockStatusFilters::default(),
    )
    .await?;
    let mut emitted_signal_count = 0usize;
    let mut suppressed_signal_count = 0usize;
    let mut redis_error_count = 0usize;
    let mut signals = Vec::new();

    for status in report.policies.iter().filter(|status| {
        status.needs_replenishment && status.policy.signal_enabled && status.policy.enabled
    }) {
        if signal_suppressed_by_cooldown(&status.policy, &status.signal_key) {
            suppressed_signal_count += 1;
            continue;
        }
        let mut event = insert_signal_event(
            pg_pool,
            &status.policy,
            &status.signal_key,
            status.severity,
            &status.signal_payload,
        )
        .await?;
        let mut published = false;
        match publish_signal_to_redis(&state.redis_pool, &event).await {
            Ok(()) => {
                let published_at = mark_signal_event_published(pg_pool, &event.id).await?;
                event.published_at = Some(published_at);
                published = true;
            }
            Err(error) => {
                warn!(
                    stock_class_key = %event.stock_class_key,
                    error = %error.message,
                    "publish credential stock signal failed"
                );
                redis_error_count += 1;
            }
        }
        if published {
            mark_policy_signal_sent(pg_pool, &status.policy.id, &status.signal_key).await?;
            emitted_signal_count += 1;
        }
        signals.push(event);
    }

    Ok(CredentialStockSignalSweepResult {
        scanned_policy_count: report.summary.enabled_policy_count,
        replenishment_policy_count: report.summary.needs_replenishment_policy_count,
        emitted_signal_count,
        suppressed_signal_count,
        redis_error_count,
        signals,
    })
}

async fn query_policy_rows(
    pool: &PgPool,
    filters: &CredentialStockStatusFilters,
    only_enabled: bool,
) -> Result<Vec<CredentialStockPolicyRow>, GatewayError> {
    let mut builder =
        QueryBuilder::<Postgres>::new("select * from gateway_credential_stock_policies where 1=1");
    if only_enabled {
        builder.push(" and enabled = true");
    }
    push_optional_text_filter(
        &mut builder,
        "stock_class_key",
        filters.stock_class_key.as_deref(),
    );
    push_optional_text_filter(
        &mut builder,
        "service_provider_key",
        filters.service_provider_key.as_deref(),
    );
    push_optional_text_filter(
        &mut builder,
        "implementation_line_key",
        filters.implementation_line_key.as_deref(),
    );
    push_optional_text_filter(
        &mut builder,
        "provider_surface_key",
        filters.provider_surface_key.as_deref(),
    );
    push_optional_text_filter(
        &mut builder,
        "credential_material_kind",
        filters.credential_material_kind.as_deref(),
    );
    push_optional_text_filter(
        &mut builder,
        "provider_account_id",
        filters.provider_account_id.as_deref(),
    );
    builder.push(" order by stock_class_key asc limit ");
    builder.push_bind(i64::try_from(filters.limit.unwrap_or(500).clamp(1, 2000)).unwrap_or(500));
    builder
        .build_query_as::<CredentialStockPolicyRow>()
        .fetch_all(pool)
        .await
        .map_err(map_db_error)
}

async fn evaluate_policy_row(
    pool: &PgPool,
    redis_pool: &RedisPool,
    row: CredentialStockPolicyRow,
) -> Result<CredentialStockStatusView, GatewayError> {
    let credentials = list_policy_credentials(pool, &row).await?;
    let mut usable_credential_count = 0usize;
    let mut active_credential_count = 0usize;
    let mut cooling_credential_count = 0usize;
    let mut disabled_credential_count = 0usize;
    let mut archived_credential_count = 0usize;
    let mut unknown_status_credential_count = 0usize;
    let mut available_tokens = Vec::new();

    let requires_quota_snapshot = metric_requires_quota_snapshot(&row.metric_kind);
    for credential in &credentials {
        if credential.archived_at.is_some() {
            archived_credential_count += 1;
            continue;
        }
        match credential.status.as_str() {
            "active" => {
                active_credential_count += 1;
                usable_credential_count += 1;
                if requires_quota_snapshot {
                    available_tokens.push(
                        read_cached_runtime_quota_snapshot(
                            redis_pool,
                            row.provider_account_id.as_deref().unwrap_or_default(),
                            Some(credential.provider_credential_id.as_str()),
                        )
                        .await
                        .ok()
                        .flatten()
                        .as_ref()
                        .and_then(remaining_tokens_for_policy_window),
                    );
                }
            }
            "cooling" => cooling_credential_count += 1,
            "disabled" | "inactive" => disabled_credential_count += 1,
            "archived" => archived_credential_count += 1,
            _ => unknown_status_credential_count += 1,
        }
    }

    let count_evaluation = evaluate_credential_count(
        usable_credential_count,
        CountWatermark {
            min: row
                .min_credential_count
                .and_then(|value| usize::try_from(value).ok()),
            target: row
                .target_credential_count
                .and_then(|value| usize::try_from(value).ok()),
            max: row
                .max_credential_count
                .and_then(|value| usize::try_from(value).ok()),
        },
    );
    let token_window_key = row.token_window_key.clone();
    let token_window_seconds = row.token_window_seconds;
    let token_evaluation = (row.metric_kind == "token_window").then(|| {
        evaluate_token_window(
            &available_tokens,
            TokenWindowWatermark {
                min_average_available_tokens: row.min_average_available_tokens,
                target_average_available_tokens: row.target_average_available_tokens,
                target_credential_count: row
                    .target_credential_count
                    .and_then(|value| usize::try_from(value).ok()),
            },
        )
    });
    let needs_replenishment =
        combined_needs_replenishment(&count_evaluation, token_evaluation.as_ref());
    let severity = combined_stock_severity(&count_evaluation, token_evaluation.as_ref());
    let suggested_credential_top_up_count = count_evaluation.suggested_credential_top_up_count.max(
        token_evaluation
            .as_ref()
            .map(|v| v.suggested_credential_top_up_count)
            .unwrap_or(0),
    );
    let signal_key = build_signal_key(
        &row.stock_class_key,
        &row.metric_kind,
        Some(suggested_credential_top_up_count),
        row.token_window_key.as_deref(),
    );
    let policy = policy_view_from_row(row);
    let signal_payload = serde_json::json!({
        "eventType": "gateway.credential_stock.replenishment_needed",
        "stockClassKey": policy.stock_class_key,
        "serviceProviderKey": policy.service_provider_key,
        "implementationLineKey": policy.implementation_line_key,
        "providerSurfaceKey": policy.provider_surface_key,
        "credentialMaterialKind": policy.credential_material_kind,
        "providerAccountId": policy.provider_account_id,
        "providerAdapter": policy.provider_adapter,
        "providerSupplyMetricKind": policy.provider_supply_metric_kind,
        "metricKind": policy.metric_kind,
        "severity": severity,
        "needsReplenishment": needs_replenishment,
        "usableCredentialCount": usable_credential_count,
        "suggestedCredentialTopUpCount": suggested_credential_top_up_count,
        "countEvaluation": count_evaluation,
        "tokenEvaluation": token_evaluation,
        "signalKey": signal_key,
    });

    Ok(CredentialStockStatusView {
        policy,
        provider_supply_metric_kind: signal_payload["providerSupplyMetricKind"]
            .as_str()
            .unwrap_or("credential_count")
            .to_string(),
        metric_kind: signal_payload["metricKind"]
            .as_str()
            .unwrap_or("credential_count")
            .to_string(),
        token_window_key,
        token_window_seconds,
        usable_credential_count,
        active_credential_count,
        cooling_credential_count,
        disabled_credential_count,
        archived_credential_count,
        unknown_status_credential_count,
        count_evaluation,
        token_evaluation,
        needs_replenishment,
        severity,
        suggested_credential_top_up_count,
        signal_key,
        signal_payload,
    })
}

async fn list_policy_credentials(
    pool: &PgPool,
    policy: &CredentialStockPolicyRow,
) -> Result<Vec<CredentialStockCredentialRow>, GatewayError> {
    let mut builder = QueryBuilder::<Postgres>::new(
        r#"
        select
          c.id as provider_credential_id,
          c.status,
          c.archived_at
        from gateway_provider_credentials c
        join gateway_provider_accounts a on a.id = c.provider_account_id
        where 1=1
        "#,
    );
    if let Some(provider_account_id) = policy.provider_account_id.as_deref() {
        builder.push(" and c.provider_account_id = ");
        builder.push_bind(provider_account_id.to_string());
    }
    if let Some(provider_adapter) = policy.provider_adapter.as_deref() {
        builder.push(" and a.adapter = ");
        builder.push_bind(provider_adapter.to_string());
    }
    builder.push(" and a.service_provider_key = ");
    builder.push_bind(policy.service_provider_key.clone());
    builder.push(" and a.protocol_profile = ");
    builder.push_bind(policy.provider_surface_key.clone());
    if policy.credential_material_kind != "*" && policy.credential_material_kind != "any" {
        builder.push(" and coalesce(c.payload_inline->>'credentialMaterialKind', c.payload_inline->>'credential_material_kind', '') = ");
        builder.push_bind(policy.credential_material_kind.clone());
    }
    builder.push(" order by c.created_at asc");

    builder
        .build_query_as::<CredentialStockCredentialRow>()
        .fetch_all(pool)
        .await
        .map_err(map_db_error)
}

fn remaining_tokens_for_policy_window(snapshot: &GatewayProviderQuotaView) -> Option<i64> {
    find_numeric_field(
        &snapshot.raw_data,
        &[
            "remainingTokens",
            "remaining_tokens",
            "availableTokens",
            "available_tokens",
            "remaining",
        ],
    )
    .or_else(|| {
        snapshot
            .windows
            .iter()
            .filter_map(|window| {
                let ratio = window.remaining_ratio?;
                let limit = find_numeric_field(
                    &snapshot.raw_data,
                    &[
                        "tokenLimit",
                        "token_limit",
                        "totalTokens",
                        "total_tokens",
                        "limit",
                    ],
                )?;
                Some((ratio * limit as f64).round() as i64)
            })
            .min()
    })
}

fn metric_requires_quota_snapshot(metric_kind: &str) -> bool {
    metric_kind.trim() == "token_window"
}

fn combined_needs_replenishment(
    count_evaluation: &CountStockEvaluation,
    token_evaluation: Option<&TokenWindowStockEvaluation>,
) -> bool {
    count_evaluation.needs_replenishment
        || token_evaluation
            .map(|evaluation| evaluation.needs_replenishment)
            .unwrap_or(false)
}

fn combined_stock_severity(
    count_evaluation: &CountStockEvaluation,
    token_evaluation: Option<&TokenWindowStockEvaluation>,
) -> StockSeverity {
    token_evaluation
        .map(|evaluation| max_severity(count_evaluation.severity, evaluation.severity))
        .unwrap_or(count_evaluation.severity)
}

fn find_numeric_field(value: &Value, keys: &[&str]) -> Option<i64> {
    match value {
        Value::Object(map) => {
            for key in keys {
                if let Some(value) = map.get(*key).and_then(value_to_i64) {
                    return Some(value);
                }
            }
            map.values()
                .find_map(|nested| find_numeric_field(nested, keys))
        }
        Value::Array(values) => values
            .iter()
            .find_map(|nested| find_numeric_field(nested, keys)),
        _ => None,
    }
}

fn value_to_i64(value: &Value) -> Option<i64> {
    value
        .as_i64()
        .or_else(|| value.as_u64().and_then(|value| i64::try_from(value).ok()))
        .or_else(|| value.as_f64().map(|value| value.round() as i64))
}

fn build_stock_summary(statuses: &[CredentialStockStatusView]) -> CredentialStockSummaryView {
    CredentialStockSummaryView {
        total_policy_count: statuses.len(),
        enabled_policy_count: statuses
            .iter()
            .filter(|status| status.policy.enabled)
            .count(),
        needs_replenishment_policy_count: statuses
            .iter()
            .filter(|status| status.needs_replenishment)
            .count(),
        critical_policy_count: statuses
            .iter()
            .filter(|status| status.severity == StockSeverity::Critical)
            .count(),
        warning_policy_count: statuses
            .iter()
            .filter(|status| status.severity == StockSeverity::Warning)
            .count(),
        healthy_policy_count: statuses
            .iter()
            .filter(|status| status.severity == StockSeverity::Healthy)
            .count(),
        overstock_policy_count: statuses
            .iter()
            .filter(|status| status.severity == StockSeverity::Overstock)
            .count(),
        total_suggested_credential_top_up_count: statuses
            .iter()
            .map(|status| status.suggested_credential_top_up_count)
            .sum(),
    }
}

fn signal_suppressed_by_cooldown(policy: &CredentialStockPolicyView, signal_key: &str) -> bool {
    if policy.last_signal_key.as_deref() != Some(signal_key) {
        return false;
    }
    let Some(last_signal_at) = policy.last_signal_at.as_deref().and_then(|value| {
        OffsetDateTime::parse(value, &time::format_description::well_known::Rfc3339).ok()
    }) else {
        return false;
    };
    let cooldown = time::Duration::seconds(policy.signal_cooldown_secs.max(0));
    OffsetDateTime::now_utc() < last_signal_at + cooldown
}

async fn insert_signal_event(
    pool: &PgPool,
    policy: &CredentialStockPolicyView,
    signal_key: &str,
    severity: StockSeverity,
    payload: &Value,
) -> Result<CredentialStockSignalEventView, GatewayError> {
    let id = format!("credential_stock_signal_{}", uuid::Uuid::new_v4());
    let now = OffsetDateTime::now_utc();
    let row = sqlx::query_as::<_, CredentialStockSignalEventRow>(
        r#"
        insert into gateway_credential_stock_signal_events (
          id, policy_id, stock_class_key, signal_key, severity, stream,
          payload, published_at, created_at
        ) values ($1, $2, $3, $4, $5, $6, $7, null, $8)
        returning *
        "#,
    )
    .bind(id)
    .bind(policy.id.as_str())
    .bind(policy.stock_class_key.as_str())
    .bind(signal_key)
    .bind(format!("{:?}", severity).to_ascii_lowercase())
    .bind(policy.signal_stream.as_str())
    .bind(sqlx::types::Json(payload.clone()))
    .bind(now)
    .fetch_one(pool)
    .await
    .map_err(map_db_error)?;
    Ok(signal_event_view_from_row(row))
}

async fn publish_signal_to_redis(
    redis_pool: &RedisPool,
    event: &CredentialStockSignalEventView,
) -> Result<(), GatewayError> {
    let mut conn = redis_pool
        .get()
        .await
        .map_err(|error| GatewayError::server_error(format!("get redis connection: {error}")))?;
    let payload = serde_json::to_string(&event.payload).map_err(|error| {
        GatewayError::server_error(format!(
            "serialize credential stock signal payload: {error}"
        ))
    })?;
    let _: String = redis::cmd("XADD")
        .arg(event.stream.as_str())
        .arg("*")
        .arg("eventId")
        .arg(event.id.as_str())
        .arg("stockClassKey")
        .arg(event.stock_class_key.as_str())
        .arg("signalKey")
        .arg(event.signal_key.as_str())
        .arg("severity")
        .arg(format!("{:?}", event.severity).to_ascii_lowercase())
        .arg("payload")
        .arg(payload)
        .query_async(&mut conn)
        .await
        .map_err(|error| {
            GatewayError::server_error(format!("publish credential stock signal: {error}"))
        })?;
    Ok(())
}

async fn mark_policy_signal_sent(
    pool: &PgPool,
    policy_id: &str,
    signal_key: &str,
) -> Result<(), GatewayError> {
    sqlx::query(
        r#"
        update gateway_credential_stock_policies
        set last_signal_key = $2, last_signal_at = $3, updated_at = $3
        where id = $1
        "#,
    )
    .bind(policy_id)
    .bind(signal_key)
    .bind(OffsetDateTime::now_utc())
    .execute(pool)
    .await
    .map_err(map_db_error)?;
    Ok(())
}

async fn mark_signal_event_published(
    pool: &PgPool,
    event_id: &str,
) -> Result<String, GatewayError> {
    let published_at = OffsetDateTime::now_utc();
    sqlx::query(
        r#"
        update gateway_credential_stock_signal_events
        set published_at = $2
        where id = $1
        "#,
    )
    .bind(event_id)
    .bind(published_at)
    .execute(pool)
    .await
    .map_err(map_db_error)?;
    Ok(format_timestamp(published_at))
}

fn policy_view_from_row(row: CredentialStockPolicyRow) -> CredentialStockPolicyView {
    CredentialStockPolicyView {
        id: row.id,
        stock_class_key: row.stock_class_key,
        display_name: row.display_name,
        service_provider_key: row.service_provider_key,
        implementation_line_key: row.implementation_line_key,
        provider_surface_key: row.provider_surface_key,
        credential_material_kind: row.credential_material_kind,
        provider_account_id: row.provider_account_id,
        provider_adapter: row.provider_adapter,
        selector: row.selector.0,
        provider_supply_metric_kind: row.metric_kind.clone(),
        metric_kind: row.metric_kind,
        token_window_key: row.token_window_key,
        token_window_seconds: row.token_window_seconds,
        min_credential_count: row.min_credential_count,
        target_credential_count: row.target_credential_count,
        max_credential_count: row.max_credential_count,
        min_average_available_tokens: row.min_average_available_tokens,
        target_average_available_tokens: row.target_average_available_tokens,
        signal_enabled: row.signal_enabled,
        signal_stream: row.signal_stream,
        signal_cooldown_secs: row.signal_cooldown_secs,
        enabled: row.enabled,
        last_signal_key: row.last_signal_key,
        last_signal_at: row.last_signal_at.map(format_timestamp),
        created_at: format_timestamp(row.created_at),
        updated_at: format_timestamp(row.updated_at),
    }
}

fn signal_event_view_from_row(
    row: CredentialStockSignalEventRow,
) -> CredentialStockSignalEventView {
    CredentialStockSignalEventView {
        id: row.id,
        policy_id: row.policy_id,
        stock_class_key: row.stock_class_key,
        signal_key: row.signal_key,
        severity: match row.severity.as_str() {
            "critical" => StockSeverity::Critical,
            "warning" => StockSeverity::Warning,
            "overstock" => StockSeverity::Overstock,
            _ => StockSeverity::Healthy,
        },
        stream: row.stream,
        payload: row.payload.0,
        published_at: row.published_at.map(format_timestamp),
        created_at: format_timestamp(row.created_at),
    }
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

fn resolve_provider_supply_metric_kind(
    provider_supply_metric_kind: Option<&str>,
    legacy_metric_kind: Option<&str>,
) -> Result<String, GatewayError> {
    match (provider_supply_metric_kind, legacy_metric_kind) {
        (Some(provider_supply_metric_kind), Some(legacy_metric_kind)) => {
            let normalized_provider_supply_metric_kind =
                normalize_provider_supply_metric_kind(provider_supply_metric_kind)?;
            let normalized_legacy_metric_kind =
                normalize_provider_supply_metric_kind(legacy_metric_kind)?;
            if normalized_provider_supply_metric_kind != normalized_legacy_metric_kind {
                return Err(GatewayError::bad_request(
                    "providerSupplyMetricKind 与 legacy metricKind 不一致",
                ));
            }
            Ok(normalized_provider_supply_metric_kind)
        }
        (Some(provider_supply_metric_kind), None) => {
            normalize_provider_supply_metric_kind(provider_supply_metric_kind)
        }
        (None, Some(legacy_metric_kind)) => {
            normalize_provider_supply_metric_kind(legacy_metric_kind)
        }
        (None, None) => Err(GatewayError::bad_request(
            "providerSupplyMetricKind 不能为空",
        )),
    }
}

fn normalize_provider_supply_metric_kind(value: &str) -> Result<String, GatewayError> {
    let normalized = normalize_required_key(value, "providerSupplyMetricKind", 80)?;
    match normalized.as_str() {
        "credential_count" | "token_window" => Ok(normalized),
        _ => Err(GatewayError::bad_request(
            "providerSupplyMetricKind 只能是 credential_count 或 token_window",
        )),
    }
}

fn normalize_required_key(
    value: &str,
    label: &str,
    max_len: usize,
) -> Result<String, GatewayError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(GatewayError::bad_request(format!("{label} 不能为空")));
    }
    if trimmed.len() > max_len {
        return Err(GatewayError::bad_request(format!(
            "{label} 不能超过 {max_len} 个字符"
        )));
    }
    let normalized = trimmed
        .chars()
        .map(|ch| match ch {
            'A'..='Z' => ch.to_ascii_lowercase(),
            'a'..='z' | '0'..='9' | '_' | '-' | '.' | ':' => ch,
            _ => '_',
        })
        .collect::<String>()
        .trim_matches('_')
        .to_string();
    if normalized.is_empty() {
        return Err(GatewayError::bad_request(format!(
            "{label} 不能只包含非法字符"
        )));
    }
    Ok(normalized)
}

fn normalize_credential_material_kind(value: &str) -> Result<String, GatewayError> {
    let trimmed = value.trim();
    if trimmed == "*" {
        return Ok("*".to_string());
    }
    normalize_required_key(trimmed, "credentialMaterialKind", 120)
}

fn normalize_required_text(
    value: &str,
    label: &str,
    max_len: usize,
) -> Result<String, GatewayError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(GatewayError::bad_request(format!("{label} 不能为空")));
    }
    if trimmed.chars().count() > max_len {
        return Err(GatewayError::bad_request(format!(
            "{label} 不能超过 {max_len} 个字符"
        )));
    }
    Ok(trimmed.to_string())
}

fn normalize_optional_key(
    value: Option<&str>,
    label: &str,
    max_len: usize,
) -> Result<Option<String>, GatewayError> {
    value
        .and_then(trim_nonempty)
        .map(|value| normalize_required_key(value, label, max_len))
        .transpose()
}

fn trim_nonempty(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then_some(trimmed)
}

fn max_severity(left: StockSeverity, right: StockSeverity) -> StockSeverity {
    if severity_rank(right) > severity_rank(left) {
        right
    } else {
        left
    }
}

fn severity_rank(value: StockSeverity) -> u8 {
    match value {
        StockSeverity::Healthy => 0,
        StockSeverity::Overstock => 1,
        StockSeverity::Warning => 2,
        StockSeverity::Critical => 3,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal_policy_view_for_test(
        provider_supply_metric_kind: &str,
    ) -> CredentialStockPolicyView {
        CredentialStockPolicyView {
            id: "policy-1".to_string(),
            stock_class_key: "chatgpt:web_reverse:chat:session_auth:free".to_string(),
            display_name: "ChatGPT Web Reverse Free".to_string(),
            service_provider_key: "chatgpt_platform".to_string(),
            implementation_line_key: "chatgpt_web_reverse".to_string(),
            provider_surface_key: "chatgpt_web_reverse".to_string(),
            credential_material_kind: "session_auth".to_string(),
            provider_account_id: None,
            provider_adapter: None,
            selector: serde_json::json!({}),
            provider_supply_metric_kind: provider_supply_metric_kind.to_string(),
            metric_kind: provider_supply_metric_kind.to_string(),
            token_window_key: None,
            token_window_seconds: None,
            min_credential_count: Some(1),
            target_credential_count: Some(2),
            max_credential_count: None,
            min_average_available_tokens: None,
            target_average_available_tokens: None,
            signal_enabled: true,
            signal_stream: DEFAULT_SIGNAL_STREAM.to_string(),
            signal_cooldown_secs: 300,
            enabled: true,
            last_signal_key: None,
            last_signal_at: None,
            created_at: "2026-05-31T00:00:00Z".to_string(),
            updated_at: "2026-05-31T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn count_policy_reports_target_deficit_and_critical_severity() {
        let status = evaluate_credential_count(
            2,
            CountWatermark {
                min: Some(3),
                target: Some(5),
                max: Some(10),
            },
        );

        assert!(status.needs_replenishment);
        assert_eq!(status.severity, StockSeverity::Critical);
        assert_eq!(status.deficit_to_min, 1);
        assert_eq!(status.deficit_to_target, 3);
        assert_eq!(status.suggested_credential_top_up_count, 3);
        assert_eq!(status.excess_over_max, 0);
    }

    #[test]
    fn count_policy_reports_overstock_without_replenishment() {
        let status = evaluate_credential_count(
            12,
            CountWatermark {
                min: Some(3),
                target: Some(5),
                max: Some(10),
            },
        );

        assert!(!status.needs_replenishment);
        assert_eq!(status.severity, StockSeverity::Overstock);
        assert_eq!(status.deficit_to_min, 0);
        assert_eq!(status.deficit_to_target, 0);
        assert_eq!(status.suggested_credential_top_up_count, 0);
        assert_eq!(status.excess_over_max, 2);
    }

    #[test]
    fn token_policy_reports_average_token_deficit() {
        let status = evaluate_token_window(
            &[Some(800), Some(1200), None],
            TokenWindowWatermark {
                min_average_available_tokens: Some(1_500),
                target_average_available_tokens: Some(2_000),
                target_credential_count: Some(4),
            },
        );

        assert!(status.needs_replenishment);
        assert_eq!(status.severity, StockSeverity::Critical);
        assert_eq!(status.known_token_credential_count, 2);
        assert_eq!(status.unknown_token_credential_count, 1);
        assert_eq!(status.average_available_tokens, Some(1_000));
        assert_eq!(status.deficit_to_min_average_tokens, 500);
        assert_eq!(status.deficit_to_target_average_tokens, 1_000);
        assert_eq!(status.suggested_credential_top_up_count, 2);
    }

    #[test]
    fn signal_key_is_stable_for_same_deficit_bucket() {
        let left = build_signal_key("chatgpt-web-a", "credential_count", Some(3), None);
        let right = build_signal_key("chatgpt-web-a", "credential_count", Some(3), None);

        assert_eq!(left, right);
        assert_ne!(
            left,
            build_signal_key("chatgpt-web-a", "credential_count", Some(4), None)
        );
    }

    #[test]
    fn credential_count_metric_does_not_need_quota_snapshot_reads() {
        assert!(!metric_requires_quota_snapshot("credential_count"));
        assert!(metric_requires_quota_snapshot("token_window"));
    }

    #[test]
    fn normalize_required_key_rejects_values_that_normalize_to_empty() {
        let error = normalize_required_key(" !!! ", "stockClassKey", 160)
            .expect_err("punctuation-only values must not normalize to empty keys");

        assert_eq!(error.http_status, Some(400));
    }

    #[test]
    fn credential_material_kind_allows_wildcard_selector() {
        assert_eq!(normalize_credential_material_kind("*").unwrap(), "*");
        assert_eq!(normalize_credential_material_kind("ANY").unwrap(), "any");
    }

    #[test]
    fn token_window_policy_preserves_count_replenishment_need() {
        let count = evaluate_credential_count(
            1,
            CountWatermark {
                min: Some(2),
                target: Some(2),
                max: None,
            },
        );
        let token = evaluate_token_window(
            &[Some(10_000)],
            TokenWindowWatermark {
                min_average_available_tokens: Some(1),
                target_average_available_tokens: Some(1),
                target_credential_count: None,
            },
        );

        assert!(!token.needs_replenishment);
        assert!(combined_needs_replenishment(&count, Some(&token)));
        assert_eq!(
            combined_stock_severity(&count, Some(&token)),
            StockSeverity::Critical
        );
    }

    #[test]
    fn normalize_optional_key_rejects_invalid_nonempty_values() {
        let error = normalize_optional_key(Some(" !!! "), "providerAccountId", 160)
            .expect_err("invalid nonempty optional keys must not silently become None");

        assert_eq!(error.http_status, Some(400));
    }

    #[test]
    fn normalize_optional_key_keeps_empty_values_absent() {
        assert_eq!(
            normalize_optional_key(Some("   "), "providerAccountId", 160).unwrap(),
            None
        );
        assert_eq!(
            normalize_optional_key(None, "providerAccountId", 160).unwrap(),
            None
        );
    }

    #[test]
    fn upsert_input_accepts_provider_supply_metric_kind_as_canonical_field() {
        let input: UpsertCredentialStockPolicyInput = serde_json::from_value(serde_json::json!({
            "stockClassKey": "chatgpt:web_reverse:chat:session_auth:free",
            "displayName": "ChatGPT Web Reverse Free",
            "serviceProviderKey": "chatgpt_platform",
            "implementationLineKey": "chatgpt_web_reverse",
            "providerSurfaceKey": "chatgpt_web_reverse",
            "credentialMaterialKind": "session_auth",
            "providerSupplyMetricKind": "credential_count"
        }))
        .expect("providerSupplyMetricKind is the canonical provider stock metric field");

        assert_eq!(
            resolve_provider_supply_metric_kind(
                input.provider_supply_metric_kind.as_deref(),
                input.legacy_metric_kind.as_deref(),
            )
            .unwrap(),
            "credential_count"
        );
    }

    #[test]
    fn upsert_input_keeps_legacy_metric_kind_alias_for_compatibility() {
        let input: UpsertCredentialStockPolicyInput = serde_json::from_value(serde_json::json!({
            "stockClassKey": "chatgpt:web_reverse:chat:session_auth:free",
            "displayName": "ChatGPT Web Reverse Free",
            "serviceProviderKey": "chatgpt_platform",
            "implementationLineKey": "chatgpt_web_reverse",
            "providerSurfaceKey": "chatgpt_web_reverse",
            "credentialMaterialKind": "session_auth",
            "metricKind": "token_window"
        }))
        .expect("legacy metricKind remains accepted as a compatibility alias");

        assert_eq!(
            resolve_provider_supply_metric_kind(
                input.provider_supply_metric_kind.as_deref(),
                input.legacy_metric_kind.as_deref(),
            )
            .unwrap(),
            "token_window"
        );
    }

    #[test]
    fn upsert_input_allows_round_tripped_canonical_and_legacy_alias_when_equal() {
        let input: UpsertCredentialStockPolicyInput = serde_json::from_value(serde_json::json!({
            "stockClassKey": "chatgpt:web_reverse:chat:session_auth:free",
            "displayName": "ChatGPT Web Reverse Free",
            "serviceProviderKey": "chatgpt_platform",
            "implementationLineKey": "chatgpt_web_reverse",
            "providerSurfaceKey": "chatgpt_web_reverse",
            "credentialMaterialKind": "session_auth",
            "providerSupplyMetricKind": "credential_count",
            "metricKind": "credential_count"
        }))
        .expect("round-tripped output may contain both canonical field and legacy alias");

        assert_eq!(
            resolve_provider_supply_metric_kind(
                input.provider_supply_metric_kind.as_deref(),
                input.legacy_metric_kind.as_deref(),
            )
            .unwrap(),
            "credential_count"
        );
    }

    #[test]
    fn provider_supply_metric_kind_rejects_conflicting_legacy_alias() {
        let error =
            resolve_provider_supply_metric_kind(Some("credential_count"), Some("token_window"))
                .expect_err("conflicting canonical and legacy stock metrics must be rejected");

        assert_eq!(error.http_status, Some(400));
    }

    #[test]
    fn policy_view_serializes_provider_supply_metric_kind_with_legacy_alias() {
        let policy = minimal_policy_view_for_test("credential_count");
        let value = serde_json::to_value(policy).expect("serialize policy view");

        assert_eq!(
            value["providerSupplyMetricKind"].as_str(),
            Some("credential_count")
        );
        assert_eq!(value["metricKind"].as_str(), Some("credential_count"));
    }
}
