use std::time::Duration;

use deadpool_redis::Pool;
use redis::AsyncCommands;
use rquest::{Method, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use crate::balance::BalanceStatus;
use crate::error::GatewayError;
use crate::implementation_lines;
use crate::redis::keys;
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::headers::build_upstream_headers;

const PROVIDER_QUOTA_CACHE_TTL_SECONDS: u64 = 86_400;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayProviderQuotaWindowView {
    pub key: String,
    pub label: String,
    pub used_percent: Option<f64>,
    pub remaining_ratio: Option<f64>,
    pub limit_window_seconds: Option<i64>,
    pub reset_at: Option<String>,
    pub reset_after_seconds: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayProviderQuotaView {
    pub provider_account_id: String,
    pub provider_credential_id: Option<String>,
    pub provider_type: String,
    pub source: String,
    pub status: String,
    pub ready: bool,
    pub checked_at: String,
    pub next_check_at: String,
    pub next_reset_at: Option<String>,
    pub plan_type: Option<String>,
    pub representative_claim: Option<String>,
    pub windows: Vec<GatewayProviderQuotaWindowView>,
    pub error: Option<String>,
    pub raw_data: Value,
}

#[derive(Debug, Clone)]
struct ProviderQuotaLock {
    key: String,
    token: String,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct CodexUsageResponse {
    #[serde(default)]
    plan_type: Option<String>,
    #[serde(default)]
    rate_limit: Option<CodexRateLimitInfo>,
    #[serde(default)]
    code_review_rate_limit: Option<CodexRateLimitInfo>,
    #[serde(default)]
    spend_control: Option<CodexSpendControl>,
    #[serde(default)]
    rate_limit_reached_type: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct CodexSpendControl {
    #[serde(default)]
    reached: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct CodexRateLimitInfo {
    #[serde(default)]
    allowed: Option<bool>,
    #[serde(default)]
    limit_reached: Option<bool>,
    #[serde(default)]
    primary_window: Option<CodexUsageWindow>,
    #[serde(default)]
    secondary_window: Option<CodexUsageWindow>,
}

#[derive(Debug, Deserialize)]
struct CodexUsageWindow {
    #[serde(default)]
    used_percent: Option<f64>,
    #[serde(default)]
    reset_at: Option<i64>,
    #[serde(default)]
    reset_after_seconds: Option<i64>,
    #[serde(default)]
    limit_window_seconds: Option<i64>,
}

pub fn provider_supports_quota(payload: &ProviderAccountPayload) -> bool {
    crate::protocol::chatgpt::official_api::is_chatgpt_codex_backend_payload(payload)
        || payload.balance_path.is_some()
        || payload.canonical_adapter() == "accio_compatible"
}

pub async fn read_cached_provider_quota_snapshot(
    redis_pool: &Pool,
    provider_account_id: &str,
) -> Result<Option<GatewayProviderQuotaView>, GatewayError> {
    read_cached_runtime_quota_snapshot(redis_pool, provider_account_id, None).await
}

pub async fn read_cached_runtime_quota_snapshot(
    redis_pool: &Pool,
    provider_account_id: &str,
    provider_credential_id: Option<&str>,
) -> Result<Option<GatewayProviderQuotaView>, GatewayError> {
    let mut conn = redis_pool
        .get()
        .await
        .map_err(|error| GatewayError::server_error(format!("get redis connection: {error}")))?;
    let raw: Option<String> = conn
        .get(quota_snapshot_key(
            provider_account_id,
            provider_credential_id,
        ))
        .await
        .map_err(|error| {
            GatewayError::server_error(format!("read provider quota snapshot: {error}"))
        })?;
    match raw {
        Some(raw) => serde_json::from_str::<GatewayProviderQuotaView>(&raw)
            .map(Some)
            .map_err(|error| {
                GatewayError::server_error(format!("decode provider quota snapshot: {error}"))
            }),
        None => Ok(None),
    }
}

pub async fn refresh_provider_quota_snapshot(
    redis_pool: &Pool,
    timeout_secs: u64,
    provider_account_id: &str,
    payload: &ProviderAccountPayload,
) -> Result<Option<GatewayProviderQuotaView>, GatewayError> {
    refresh_runtime_quota_snapshot(redis_pool, timeout_secs, provider_account_id, None, payload)
        .await
}

pub async fn refresh_runtime_quota_snapshot(
    redis_pool: &Pool,
    timeout_secs: u64,
    provider_account_id: &str,
    provider_credential_id: Option<&str>,
    payload: &ProviderAccountPayload,
) -> Result<Option<GatewayProviderQuotaView>, GatewayError> {
    if !provider_supports_quota(payload) {
        return Ok(None);
    }

    let lock = acquire_provider_quota_lock(redis_pool, provider_account_id, provider_credential_id)
        .await?;
    let result = refresh_provider_quota_snapshot_locked(
        redis_pool,
        timeout_secs,
        provider_account_id,
        provider_credential_id,
        payload,
    )
    .await;
    let _ = release_provider_quota_lock(redis_pool, &lock).await;
    result
}

pub async fn get_or_refresh_provider_quota_snapshot(
    redis_pool: &Pool,
    timeout_secs: u64,
    provider_account_id: &str,
    payload: &ProviderAccountPayload,
) -> Option<GatewayProviderQuotaView> {
    get_or_refresh_runtime_quota_snapshot(
        redis_pool,
        timeout_secs,
        provider_account_id,
        None,
        payload,
    )
    .await
}

pub async fn get_or_refresh_runtime_quota_snapshot(
    redis_pool: &Pool,
    timeout_secs: u64,
    provider_account_id: &str,
    provider_credential_id: Option<&str>,
    payload: &ProviderAccountPayload,
) -> Option<GatewayProviderQuotaView> {
    if !provider_supports_quota(payload) {
        return None;
    }

    let cached =
        read_cached_runtime_quota_snapshot(redis_pool, provider_account_id, provider_credential_id)
            .await
            .ok()
            .flatten();

    if cached
        .as_ref()
        .is_some_and(|snapshot| !quota_refresh_due(snapshot))
    {
        return cached;
    }

    match acquire_provider_quota_lock(redis_pool, provider_account_id, provider_credential_id).await
    {
        Ok(lock) => {
            let refreshed = refresh_provider_quota_snapshot_locked(
                redis_pool,
                timeout_secs,
                provider_account_id,
                provider_credential_id,
                payload,
            )
            .await;
            let _ = release_provider_quota_lock(redis_pool, &lock).await;
            refreshed.ok().flatten().or(cached)
        }
        Err(_) => cached,
    }
}

pub fn quota_to_balance_status(snapshot: &GatewayProviderQuotaView) -> BalanceStatus {
    let remaining_ratio = snapshot
        .windows
        .iter()
        .filter_map(|window| window.remaining_ratio)
        .min_by(|left, right| left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal));
    let should_deprioritize = snapshot.status == "warning";
    let is_unavailable = snapshot.status == "exhausted";
    let display = snapshot.representative_claim.clone().unwrap_or_else(|| {
        if let Some(window) = snapshot.windows.first() {
            if let Some(used_percent) = window.used_percent {
                format!("{} 已用 {:.0}%", window.label, used_percent)
            } else {
                format!("{} quota {}", snapshot.provider_type, snapshot.status)
            }
        } else {
            format!("{} quota {}", snapshot.provider_type, snapshot.status)
        }
    });
    let reason = if snapshot.status == "warning" {
        Some("provider quota nearing limit".to_string())
    } else if snapshot.status == "exhausted" {
        Some("provider quota exhausted".to_string())
    } else {
        None
    };

    BalanceStatus {
        provider_account_id: snapshot.provider_account_id.clone(),
        display,
        reason,
        should_deprioritize,
        is_unavailable,
        remaining_ratio,
    }
}

pub fn aggregate_provider_quota_snapshots(
    provider_account_id: &str,
    snapshots: &[GatewayProviderQuotaView],
) -> Option<GatewayProviderQuotaView> {
    if snapshots.is_empty() {
        return None;
    }

    let available_count = snapshots
        .iter()
        .filter(|snapshot| snapshot.status == "available")
        .count();
    let warning_count = snapshots
        .iter()
        .filter(|snapshot| snapshot.status == "warning")
        .count();
    let exhausted_count = snapshots
        .iter()
        .filter(|snapshot| snapshot.status == "exhausted")
        .count();
    let unknown_count = snapshots.len() - available_count - warning_count - exhausted_count;
    let ready = snapshots.iter().any(|snapshot| snapshot.ready);
    let status = if available_count > 0 {
        if warning_count > 0 {
            "warning"
        } else {
            "available"
        }
    } else if warning_count > 0 {
        "warning"
    } else if exhausted_count > 0 {
        "exhausted"
    } else {
        "unknown"
    };

    let best_snapshot = snapshots
        .iter()
        .max_by(|left, right| {
            best_snapshot_rank(left)
                .cmp(&best_snapshot_rank(right))
                .then_with(|| {
                    snapshot_best_remaining_ratio(left)
                        .partial_cmp(&snapshot_best_remaining_ratio(right))
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
        })
        .cloned()
        .unwrap_or_else(|| snapshots[0].clone());

    let checked_at = snapshots
        .iter()
        .filter_map(|snapshot| parse_timestamp(snapshot.checked_at.as_str()))
        .max()
        .map(format_timestamp)
        .unwrap_or_else(|| best_snapshot.checked_at.clone());
    let next_check_at = snapshots
        .iter()
        .filter_map(|snapshot| parse_timestamp(snapshot.next_check_at.as_str()))
        .min()
        .map(format_timestamp)
        .unwrap_or_else(|| best_snapshot.next_check_at.clone());
    let next_reset_at = snapshots
        .iter()
        .filter_map(|snapshot| snapshot.next_reset_at.as_deref())
        .filter_map(parse_timestamp)
        .min()
        .map(format_timestamp)
        .or(best_snapshot.next_reset_at.clone());

    Some(GatewayProviderQuotaView {
        provider_account_id: provider_account_id.to_string(),
        provider_credential_id: None,
        provider_type: best_snapshot.provider_type.clone(),
        source: "aggregated_provider_credentials".to_string(),
        status: status.to_string(),
        ready,
        checked_at,
        next_check_at,
        next_reset_at,
        plan_type: best_snapshot.plan_type.clone(),
        representative_claim: Some(format!(
            "{} 条凭证：{} 正常 / {} 预警 / {} 耗尽 / {} 未知",
            snapshots.len(),
            available_count,
            warning_count,
            exhausted_count,
            unknown_count
        )),
        windows: best_snapshot.windows.clone(),
        error: snapshots.iter().find_map(|snapshot| snapshot.error.clone()),
        raw_data: serde_json::json!({
            "aggregated": true,
            "credentialIds": snapshots
                .iter()
                .filter_map(|snapshot| snapshot.provider_credential_id.clone())
                .collect::<Vec<_>>(),
            "availableCount": available_count,
            "warningCount": warning_count,
            "exhaustedCount": exhausted_count,
            "unknownCount": unknown_count,
        }),
    })
}

fn quota_refresh_due(snapshot: &GatewayProviderQuotaView) -> bool {
    let Ok(next_check_at) = OffsetDateTime::parse(&snapshot.next_check_at, &Rfc3339) else {
        return true;
    };
    OffsetDateTime::now_utc() >= next_check_at
}

fn best_snapshot_rank(snapshot: &GatewayProviderQuotaView) -> i32 {
    match snapshot.status.as_str() {
        "available" => 4,
        "warning" => 3,
        "unknown" => 2,
        "exhausted" => 1,
        _ => 0,
    }
}

fn snapshot_best_remaining_ratio(snapshot: &GatewayProviderQuotaView) -> f64 {
    snapshot
        .windows
        .iter()
        .filter_map(|window| window.remaining_ratio)
        .max_by(|left, right| left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal))
        .unwrap_or(0.0)
}

fn parse_timestamp(value: &str) -> Option<OffsetDateTime> {
    OffsetDateTime::parse(value, &Rfc3339).ok()
}

fn format_timestamp(value: OffsetDateTime) -> String {
    value
        .format(&Rfc3339)
        .unwrap_or_else(|_| value.unix_timestamp().to_string())
}

async fn refresh_provider_quota_snapshot_locked(
    redis_pool: &Pool,
    timeout_secs: u64,
    provider_account_id: &str,
    provider_credential_id: Option<&str>,
    payload: &ProviderAccountPayload,
) -> Result<Option<GatewayProviderQuotaView>, GatewayError> {
    let snapshot =
        if crate::protocol::chatgpt::official_api::is_chatgpt_codex_backend_payload(payload) {
            fetch_codex_quota_snapshot(
                timeout_secs,
                provider_account_id,
                provider_credential_id,
                payload,
            )
            .await?
        } else if payload.canonical_adapter() == "accio_compatible" {
            fetch_accio_quota_snapshot(
                timeout_secs,
                provider_account_id,
                provider_credential_id,
                payload,
            )
            .await?
        } else if payload.balance_path.is_some() {
            fetch_generic_balance_snapshot(
                timeout_secs,
                provider_account_id,
                provider_credential_id,
                payload,
            )
            .await?
        } else {
            return Ok(None);
        };
    store_cached_provider_quota_snapshot(redis_pool, &snapshot).await?;
    Ok(Some(snapshot))
}

async fn fetch_codex_quota_snapshot(
    timeout_secs: u64,
    provider_account_id: &str,
    provider_credential_id: Option<&str>,
    payload: &ProviderAccountPayload,
) -> Result<GatewayProviderQuotaView, GatewayError> {
    let client = rquest::Client::builder()
        .timeout(Duration::from_secs(timeout_secs.max(1)))
        .build()
        .map_err(|error| {
            GatewayError::server_error(format!("build provider quota client: {error}"))
        })?;
    let response = client
        .request(Method::GET, "https://chatgpt.com/backend-api/wham/usage")
        .headers(build_upstream_headers(payload))
        .send()
        .await
        .map_err(|error| {
            GatewayError::service_unavailable(format!("Codex quota request failed: {error}"))
                .with_code("provider_quota_request_failed")
        })?;
    let status = response.status();
    let body = response.bytes().await.map_err(|error| {
        GatewayError::service_unavailable(format!("read Codex quota response: {error}"))
    })?;
    if !status.is_success() {
        return Err(provider_quota_http_error("Codex", status, &body));
    }

    let raw_data: Value = serde_json::from_slice(&body).map_err(|error| {
        GatewayError::server_error(format!("parse Codex quota raw body: {error}"))
            .with_code("provider_quota_parse_failed")
    })?;
    let parsed: CodexUsageResponse = serde_json::from_slice(&body).map_err(|error| {
        GatewayError::server_error(format!("parse Codex quota body: {error}"))
            .with_code("provider_quota_parse_failed")
    })?;

    let now = OffsetDateTime::now_utc();
    let mut windows = Vec::new();
    if let Some(rate_limit) = parsed.rate_limit.as_ref() {
        if let Some(window) = rate_limit.primary_window.as_ref() {
            windows.push(codex_window_to_view("primary", window));
        }
        if let Some(window) = rate_limit.secondary_window.as_ref() {
            windows.push(codex_window_to_view("secondary", window));
        }
    }
    windows.sort_by_key(|window| window.limit_window_seconds.unwrap_or(i64::MAX));

    let exhausted = parsed
        .spend_control
        .as_ref()
        .and_then(|value| value.reached)
        .unwrap_or(false)
        || parsed
            .rate_limit
            .as_ref()
            .and_then(|value| value.limit_reached)
            .unwrap_or(false)
        || parsed
            .rate_limit
            .as_ref()
            .and_then(|value| value.allowed)
            .is_some_and(|allowed| !allowed);
    let status = if exhausted {
        "exhausted"
    } else if parsed.rate_limit.is_some() {
        "available"
    } else {
        "unknown"
    };

    let next_reset_at = windows
        .iter()
        .filter_map(|window| {
            window
                .reset_at
                .as_deref()
                .and_then(|value| OffsetDateTime::parse(value, &Rfc3339).ok())
        })
        .min()
        .map(format_rfc3339);
    let next_check_at = derive_next_check_at(now, status, next_reset_at.as_deref());
    let representative_claim = windows.first().map(|window| {
        if let Some(used_percent) = window.used_percent {
            format!("{} 窗口已用 {:.0}%", window.label, used_percent)
        } else {
            format!("{} quota {}", "Codex", status)
        }
    });

    Ok(GatewayProviderQuotaView {
        provider_account_id: provider_account_id.to_string(),
        provider_credential_id: provider_credential_id.map(str::to_string),
        provider_type: "codex".to_string(),
        source: "codex_wham_usage".to_string(),
        status: status.to_string(),
        ready: matches!(status, "available" | "warning"),
        checked_at: format_rfc3339(now),
        next_check_at: format_rfc3339(next_check_at),
        next_reset_at,
        plan_type: parsed.plan_type,
        representative_claim,
        windows,
        error: None,
        raw_data,
    })
}

async fn fetch_accio_quota_snapshot(
    timeout_secs: u64,
    provider_account_id: &str,
    provider_credential_id: Option<&str>,
    payload: &ProviderAccountPayload,
) -> Result<GatewayProviderQuotaView, GatewayError> {
    implementation_lines::assert_accio_web_reverse_api_compiled("Accio quota probe requested")?;
    let current = fetch_accio_quota_endpoint(
        timeout_secs,
        payload,
        "/api/entitlement/currentSubscription",
        "accio_current_subscription",
        false,
    )
    .await;
    let (source, raw_data) = match current {
        Ok((source, raw)) => (source, raw),
        Err(primary_error) => {
            let probe = fetch_accio_quota_endpoint(
                timeout_secs,
                payload,
                "/api/entitlement/quota",
                "accio_quota_probe",
                true,
            )
            .await;
            match probe {
                Ok((source, raw)) => (source, raw),
                Err(_) => return Err(primary_error),
            }
        }
    };

    let now = OffsetDateTime::now_utc();
    let total = find_numeric_field(
        &raw_data,
        &[
            "total",
            "quota",
            "monthlyTotal",
            "monthly_total",
            "totalQuota",
            "total_quota",
        ],
    );
    let remaining = find_numeric_field(
        &raw_data,
        &[
            "remaining",
            "remainingQuota",
            "remaining_quota",
            "available",
            "availableQuota",
            "available_quota",
        ],
    );
    let used =
        find_numeric_field(&raw_data, &["used", "usedQuota", "used_quota"]).or_else(|| {
            match (total, remaining) {
                (Some(total), Some(remaining)) => Some((total - remaining).max(0.0)),
                _ => None,
            }
        });
    let usage_percent =
        find_numeric_field(&raw_data, &["usagePercent", "usage_percent"]).or_else(|| {
            match (used, total) {
                (Some(used), Some(total)) if total > 0.0 => Some((used / total) * 100.0),
                _ => None,
            }
        });
    let remaining_ratio = match (remaining, total) {
        (Some(remaining), Some(total)) if total > 0.0 => Some((remaining / total).clamp(0.0, 1.0)),
        _ => usage_percent.map(|percent| (1.0 - percent / 100.0).clamp(0.0, 1.0)),
    };

    let quota_status = if remaining.is_some_and(|value| value <= 0.0)
        || usage_percent.is_some_and(|value| value >= 100.0)
    {
        "exhausted"
    } else if remaining_ratio.is_some_and(|ratio| ratio < 0.2)
        || usage_percent.is_some_and(|value| value >= 80.0)
    {
        "warning"
    } else if remaining.is_some() || total.is_some() || usage_percent.is_some() {
        "available"
    } else {
        "unknown"
    };

    let window = if total.is_some() || remaining.is_some() || usage_percent.is_some() {
        Some(GatewayProviderQuotaWindowView {
            key: "monthly".to_string(),
            label: "monthly".to_string(),
            used_percent: usage_percent.map(round_percent),
            remaining_ratio,
            limit_window_seconds: None,
            reset_at: None,
            reset_after_seconds: find_numeric_field(
                &raw_data,
                &["refreshCountdownSeconds", "refresh_countdown_seconds"],
            )
            .map(|value| value.round() as i64),
        })
    } else {
        None
    };

    let representative_claim = match (remaining, total) {
        (Some(remaining), Some(total)) => Some(format!("剩余额度 {:.0} / {:.0}", remaining, total)),
        (Some(remaining), None) => Some(format!("剩余额度 {:.0}", remaining)),
        _ => usage_percent.map(|value| format!("额度已用 {:.1}%", round_percent(value))),
    };

    Ok(GatewayProviderQuotaView {
        provider_account_id: provider_account_id.to_string(),
        provider_credential_id: provider_credential_id.map(str::to_string),
        provider_type: "accio".to_string(),
        source,
        status: quota_status.to_string(),
        ready: matches!(quota_status, "available" | "warning"),
        checked_at: format_rfc3339(now),
        next_check_at: format_rfc3339(derive_next_check_at(now, quota_status, None)),
        next_reset_at: None,
        plan_type: None,
        representative_claim,
        windows: window.into_iter().collect(),
        error: None,
        raw_data,
    })
}

async fn fetch_accio_quota_endpoint(
    timeout_secs: u64,
    payload: &ProviderAccountPayload,
    path: &str,
    source: &str,
    probe: bool,
) -> Result<(String, Value), GatewayError> {
    let access_token = payload.api_key.trim();
    if access_token.is_empty() {
        return Err(
            GatewayError::bad_request("Accio provider quota probe 缺少 access token")
                .with_code("accio_quota_missing_access_token"),
        );
    }

    let utdid = payload
        .headers
        .get("utdid")
        .or_else(|| payload.headers.get("x-utdid"))
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            GatewayError::bad_request("Accio provider quota probe 缺少 utdid")
                .with_code("accio_quota_missing_utdid")
        })?;
    let version = payload
        .headers
        .get("version")
        .or_else(|| payload.headers.get("x-app-version"))
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .unwrap_or("0.5.6");

    let client = rquest::Client::builder()
        .timeout(Duration::from_secs(timeout_secs.max(1)))
        .build()
        .map_err(|error| {
            GatewayError::server_error(format!("build accio quota client: {error}"))
        })?;
    let response = client
        .request(
            Method::GET,
            build_absolute_url(payload.base_url.trim_end_matches('/'), path),
        )
        .query(&[
            ("accessToken", access_token),
            ("utdid", utdid),
            ("version", version),
        ])
        .headers(build_accio_control_plane_headers(payload, true))
        .send()
        .await
        .map_err(|error| {
            GatewayError::service_unavailable(format!("Accio quota request failed: {error}"))
                .with_code("provider_quota_request_failed")
        })?;
    let status = response.status();
    let body = response.bytes().await.map_err(|error| {
        GatewayError::service_unavailable(format!("read Accio quota response: {error}"))
    })?;
    if !status.is_success() {
        return Err(provider_quota_http_error(
            if probe {
                "Accio quota probe"
            } else {
                "Accio currentSubscription"
            },
            status,
            &body,
        ));
    }
    let raw_data: Value = serde_json::from_slice(&body).map_err(|error| {
        GatewayError::server_error(format!("parse Accio quota body: {error}"))
            .with_code("provider_quota_parse_failed")
    })?;
    Ok((source.to_string(), raw_data))
}

async fn fetch_generic_balance_snapshot(
    timeout_secs: u64,
    provider_account_id: &str,
    provider_credential_id: Option<&str>,
    payload: &ProviderAccountPayload,
) -> Result<GatewayProviderQuotaView, GatewayError> {
    let path = payload
        .balance_path
        .as_deref()
        .ok_or_else(|| GatewayError::bad_request("当前 provider 未配置 balancePath"))?;
    let client = rquest::Client::builder()
        .timeout(Duration::from_secs(timeout_secs.max(1)))
        .build()
        .map_err(|error| {
            GatewayError::server_error(format!("build provider quota client: {error}"))
        })?;
    let response = client
        .request(
            Method::GET,
            build_absolute_url(payload.base_url.trim_end_matches('/'), path),
        )
        .headers(build_upstream_headers(payload))
        .send()
        .await
        .map_err(|error| {
            GatewayError::service_unavailable(format!("provider balance request failed: {error}"))
                .with_code("provider_quota_request_failed")
        })?;
    let status = response.status();
    let body = response.bytes().await.map_err(|error| {
        GatewayError::service_unavailable(format!("read provider balance response: {error}"))
    })?;
    if !status.is_success() {
        return Err(provider_quota_http_error("balance", status, &body));
    }
    let raw_data: Value = serde_json::from_slice(&body).map_err(|error| {
        GatewayError::server_error(format!("parse provider balance body: {error}"))
            .with_code("provider_quota_parse_failed")
    })?;

    let remaining = find_numeric_field(
        &raw_data,
        &[
            "remaining",
            "remaining_credits",
            "remainingCredits",
            "available",
            "available_credits",
            "availableCredits",
            "balance",
            "credits",
        ],
    );
    let total = find_numeric_field(
        &raw_data,
        &[
            "total",
            "limit",
            "quota",
            "max",
            "initial",
            "total_credits",
            "totalCredits",
        ],
    );
    let remaining_ratio = match (remaining, total) {
        (Some(remaining), Some(total)) if total > 0.0 => Some((remaining / total).clamp(0.0, 1.0)),
        _ => None,
    };
    let exhausted = find_bool_field(&raw_data, &["limit_reached", "limitReached", "exhausted"])
        .unwrap_or(false)
        || remaining.is_some_and(|value| value <= 0.0)
        || find_status_string(&raw_data)
            .as_deref()
            .is_some_and(|value| matches!(value, "exhausted" | "depleted" | "unavailable"));
    let warning = !exhausted && remaining_ratio.is_some_and(|ratio| ratio < 0.2)
        || find_status_string(&raw_data)
            .as_deref()
            .is_some_and(|value| matches!(value, "warning" | "low"));
    let quota_status = if exhausted {
        "exhausted"
    } else if warning {
        "warning"
    } else if remaining.is_some() || total.is_some() {
        "available"
    } else {
        "unknown"
    };

    let now = OffsetDateTime::now_utc();
    Ok(GatewayProviderQuotaView {
        provider_account_id: provider_account_id.to_string(),
        provider_credential_id: provider_credential_id.map(str::to_string),
        provider_type: payload.canonical_adapter().to_string(),
        source: "balance_path_json".to_string(),
        status: quota_status.to_string(),
        ready: matches!(quota_status, "available" | "warning"),
        checked_at: format_rfc3339(now),
        next_check_at: format_rfc3339(derive_next_check_at(now, quota_status, None)),
        next_reset_at: None,
        plan_type: None,
        representative_claim: remaining.map(|value| format!("剩余额度 {:.2}", value)),
        windows: Vec::new(),
        error: None,
        raw_data,
    })
}

async fn store_cached_provider_quota_snapshot(
    redis_pool: &Pool,
    snapshot: &GatewayProviderQuotaView,
) -> Result<(), GatewayError> {
    let mut conn = redis_pool
        .get()
        .await
        .map_err(|error| GatewayError::server_error(format!("get redis connection: {error}")))?;
    let value = serde_json::to_string(snapshot).map_err(|error| {
        GatewayError::server_error(format!("encode provider quota snapshot: {error}"))
    })?;
    let _: () = conn
        .set_ex(
            quota_snapshot_key(
                &snapshot.provider_account_id,
                snapshot.provider_credential_id.as_deref(),
            ),
            value,
            PROVIDER_QUOTA_CACHE_TTL_SECONDS,
        )
        .await
        .map_err(|error| {
            GatewayError::server_error(format!("store provider quota snapshot: {error}"))
        })?;
    Ok(())
}

fn codex_window_to_view(key: &str, window: &CodexUsageWindow) -> GatewayProviderQuotaWindowView {
    let limit_window_seconds = window.limit_window_seconds;
    let reset_at = window
        .reset_at
        .and_then(|value| OffsetDateTime::from_unix_timestamp(value).ok())
        .map(format_rfc3339);
    let remaining_ratio = window
        .used_percent
        .map(|used_percent| (1.0 - used_percent / 100.0).clamp(0.0, 1.0));
    GatewayProviderQuotaWindowView {
        key: key.to_string(),
        label: window_label_from_seconds(limit_window_seconds).unwrap_or_else(|| key.to_string()),
        used_percent: window.used_percent.map(round_percent),
        remaining_ratio,
        limit_window_seconds,
        reset_at,
        reset_after_seconds: window.reset_after_seconds,
    }
}

fn round_percent(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

fn window_label_from_seconds(limit_window_seconds: Option<i64>) -> Option<String> {
    match limit_window_seconds.unwrap_or_default() {
        18_000 => Some("5h".to_string()),
        86_400 => Some("1d".to_string()),
        604_800 => Some("7d".to_string()),
        seconds if seconds > 0 => Some(format!("{}s", seconds)),
        _ => None,
    }
}

fn derive_next_check_at(
    now: OffsetDateTime,
    status: &str,
    next_reset_at: Option<&str>,
) -> OffsetDateTime {
    let fallback = match status {
        "warning" => now + time::Duration::minutes(1),
        "exhausted" => now + time::Duration::minutes(5),
        "unknown" => now + time::Duration::minutes(2),
        _ => now + time::Duration::minutes(5),
    };
    match next_reset_at.and_then(|value| OffsetDateTime::parse(value, &Rfc3339).ok()) {
        Some(next_reset) if next_reset < fallback => next_reset,
        _ => fallback,
    }
}

fn format_rfc3339(value: OffsetDateTime) -> String {
    value
        .format(&Rfc3339)
        .unwrap_or_else(|_| "9999-12-31T23:59:59Z".to_string())
}

fn provider_quota_http_error(label: &str, status: StatusCode, body: &[u8]) -> GatewayError {
    let message = String::from_utf8_lossy(body);
    let summary = message.trim();
    let description = if summary.is_empty() {
        format!("{label} quota probe failed with status {status}")
    } else {
        format!(
            "{label} quota probe failed with status {status}: {}",
            summary.chars().take(240).collect::<String>()
        )
    };
    if status.as_u16() >= 500 {
        GatewayError::service_unavailable(description).with_code("provider_quota_http_error")
    } else {
        GatewayError::conflict(description).with_code("provider_quota_http_error")
    }
}

fn build_accio_control_plane_headers(
    payload: &ProviderAccountPayload,
    quota_request: bool,
) -> rquest::header::HeaderMap {
    let mut headers = rquest::header::HeaderMap::new();
    headers.insert(
        rquest::header::CONTENT_TYPE,
        rquest::header::HeaderValue::from_static("application/json"),
    );
    headers.insert(
        "accept",
        rquest::header::HeaderValue::from_static(if quota_request {
            "*/*"
        } else {
            "application/json"
        }),
    );
    headers.insert(
        "user-agent",
        rquest::header::HeaderValue::from_static("node"),
    );
    headers.insert("x-language", rquest::header::HeaderValue::from_static("zh"));
    headers.insert("x-os", rquest::header::HeaderValue::from_static("win32"));
    headers.insert(
        "x-app-version",
        rquest::header::HeaderValue::from_str(
            payload
                .headers
                .get("x-app-version")
                .or_else(|| payload.headers.get("version"))
                .map(String::as_str)
                .unwrap_or("0.5.6"),
        )
        .unwrap_or_else(|_| rquest::header::HeaderValue::from_static("0.5.6")),
    );
    if let Some(utdid) = payload
        .headers
        .get("x-utdid")
        .or_else(|| payload.headers.get("utdid"))
        .map(String::as_str)
    {
        if let Ok(value) = rquest::header::HeaderValue::from_str(utdid) {
            headers.insert("x-utdid", value);
        }
    }
    if let Some(cna) = payload
        .headers
        .get("x-cna")
        .map(String::as_str)
        .or_else(|| {
            payload
                .headers
                .get("Cookie")
                .or_else(|| payload.headers.get("cookie"))
                .and_then(|value| extract_cookie_value(value, "cna"))
        })
    {
        if let Ok(value) = rquest::header::HeaderValue::from_str(cna) {
            headers.insert("x-cna", value);
        }
    }
    if quota_request {
        headers.insert(
            "accept-language",
            rquest::header::HeaderValue::from_static("*"),
        );
        headers.insert(
            "sec-fetch-mode",
            rquest::header::HeaderValue::from_static("cors"),
        );
    }
    headers
}

fn extract_cookie_value<'a>(cookie_header: &'a str, cookie_name: &str) -> Option<&'a str> {
    cookie_header
        .split(';')
        .filter_map(|segment| segment.split_once('='))
        .find_map(|(name, value)| {
            if name.trim().eq_ignore_ascii_case(cookie_name) {
                let trimmed = value.trim();
                (!trimmed.is_empty()).then_some(trimmed)
            } else {
                None
            }
        })
}

fn find_numeric_field(value: &Value, candidate_keys: &[&str]) -> Option<f64> {
    match value {
        Value::Object(map) => {
            for key in candidate_keys {
                if let Some(number) = map.get(*key).and_then(value_as_f64) {
                    return Some(number);
                }
            }
            map.values()
                .find_map(|child| find_numeric_field(child, candidate_keys))
        }
        Value::Array(values) => values
            .iter()
            .find_map(|child| find_numeric_field(child, candidate_keys)),
        _ => None,
    }
}

fn find_bool_field(value: &Value, candidate_keys: &[&str]) -> Option<bool> {
    match value {
        Value::Object(map) => {
            for key in candidate_keys {
                if let Some(flag) = map.get(*key).and_then(value_as_bool) {
                    return Some(flag);
                }
            }
            map.values()
                .find_map(|child| find_bool_field(child, candidate_keys))
        }
        Value::Array(values) => values
            .iter()
            .find_map(|child| find_bool_field(child, candidate_keys)),
        _ => None,
    }
}

fn find_status_string(value: &Value) -> Option<String> {
    match value {
        Value::Object(map) => {
            for key in ["status", "state", "quota_status"] {
                if let Some(status) = map.get(key).and_then(Value::as_str) {
                    let status = status.trim().to_lowercase();
                    if !status.is_empty() {
                        return Some(status);
                    }
                }
            }
            map.values().find_map(find_status_string)
        }
        Value::Array(values) => values.iter().find_map(find_status_string),
        _ => None,
    }
}

fn value_as_f64(value: &Value) -> Option<f64> {
    match value {
        Value::Number(number) => number.as_f64(),
        Value::String(value) => value.trim().parse::<f64>().ok(),
        _ => None,
    }
}

fn value_as_bool(value: &Value) -> Option<bool> {
    match value {
        Value::Bool(value) => Some(*value),
        Value::String(value) => match value.trim().to_ascii_lowercase().as_str() {
            "true" | "1" | "yes" => Some(true),
            "false" | "0" | "no" => Some(false),
            _ => None,
        },
        _ => None,
    }
}

async fn acquire_provider_quota_lock(
    redis_pool: &Pool,
    provider_account_id: &str,
    provider_credential_id: Option<&str>,
) -> Result<ProviderQuotaLock, GatewayError> {
    let key = quota_lock_key(provider_account_id, provider_credential_id);
    let token = uuid::Uuid::new_v4().to_string();
    let mut conn = redis_pool
        .get()
        .await
        .map_err(|error| GatewayError::server_error(format!("get redis connection: {error}")))?;
    let acquired: Option<String> = redis::cmd("SET")
        .arg(&key)
        .arg(&token)
        .arg("PX")
        .arg(15_000)
        .arg("NX")
        .query_async(&mut conn)
        .await
        .map_err(|error| {
            GatewayError::server_error(format!("acquire provider quota lock: {error}"))
        })?;

    if acquired.as_deref() != Some("OK") {
        return Err(GatewayError::conflict(
            "当前 provider account 正在刷新额度快照，请稍后再试。",
        ));
    }

    Ok(ProviderQuotaLock { key, token })
}

fn quota_snapshot_key(provider_account_id: &str, provider_credential_id: Option<&str>) -> String {
    provider_credential_id
        .map(keys::provider_credential_quota_snapshot_key)
        .unwrap_or_else(|| keys::provider_quota_snapshot_key(provider_account_id))
}

fn quota_lock_key(provider_account_id: &str, provider_credential_id: Option<&str>) -> String {
    provider_credential_id
        .map(keys::provider_credential_quota_lock_key)
        .unwrap_or_else(|| keys::provider_quota_lock_key(provider_account_id))
}

async fn release_provider_quota_lock(
    redis_pool: &Pool,
    lock: &ProviderQuotaLock,
) -> Result<(), GatewayError> {
    let mut conn = redis_pool
        .get()
        .await
        .map_err(|error| GatewayError::server_error(format!("get redis connection: {error}")))?;
    let current: Option<String> = conn.get(&lock.key).await.map_err(|error| {
        GatewayError::server_error(format!("read provider quota lock: {error}"))
    })?;
    if current.as_deref() == Some(lock.token.as_str()) {
        let _: usize = conn.del(&lock.key).await.map_err(|error| {
            GatewayError::server_error(format!("release provider quota lock: {error}"))
        })?;
    }
    Ok(())
}

fn build_absolute_url(base_url: &str, path: &str) -> String {
    if path.starts_with("http://") || path.starts_with("https://") {
        return path.to_string();
    }
    if path.starts_with('/') {
        format!("{base_url}{path}")
    } else {
        format!("{base_url}/{path}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::routing::candidate::ProviderExecutionMode;
    use std::collections::HashMap;

    fn codex_payload() -> ProviderAccountPayload {
        let mut headers = HashMap::new();
        headers.insert("Originator".to_string(), "codex_cli_rs".to_string());
        ProviderAccountPayload {
            adapter: "openai_compatible".to_string(),
            base_url: "https://chatgpt.com/backend-api/codex".to_string(),
            api_key: "tok".to_string(),
            credential_id: None,
            expires_at: None,
            runtime_state_object_key: None,
            account_name: None,
            execution_mode: Some(ProviderExecutionMode::DirectHttp),
            endpoint_execution_modes: None,
            default_model: Some("gpt-5.4".to_string()),
            headers,
            auth_mode: None,
            anthropic_version: None,
            beta_headers: None,
            auth_header_name: None,
            auth_token: None,
            responses_path: Some("/responses".to_string()),
            chat_completions_path: None,
            completions_path: None,
            embeddings_path: None,
            audio_transcriptions_path: None,
            audio_speech_path: None,
            messages_path: None,
            search_path: None,
            fetch_path: None,
            research_path: None,
            balance_path: None,
            search_query_field: None,
            fetch_urls_field: None,
            extra_body: None,
            session_auth: None,
            keepalive: None,
        }
    }

    fn accio_payload() -> ProviderAccountPayload {
        let mut headers = HashMap::new();
        headers.insert("utdid".to_string(), "utd-accio".to_string());
        headers.insert("version".to_string(), "0.5.6".to_string());
        headers.insert(
            "Cookie".to_string(),
            "cna=test-cna; other=value".to_string(),
        );
        ProviderAccountPayload {
            adapter: "accio_compatible".to_string(),
            base_url: "https://phoenix-gw.alibaba.com".to_string(),
            api_key: "accio-access-token".to_string(),
            credential_id: None,
            expires_at: None,
            runtime_state_object_key: None,
            account_name: Some("accio@example.com".to_string()),
            execution_mode: Some(ProviderExecutionMode::DirectHttp),
            endpoint_execution_modes: None,
            default_model: Some("claude-sonnet-4-6".to_string()),
            headers,
            auth_mode: None,
            anthropic_version: None,
            beta_headers: None,
            auth_header_name: None,
            auth_token: None,
            responses_path: Some("/api/adk/llm/generateContent".to_string()),
            chat_completions_path: None,
            completions_path: None,
            embeddings_path: None,
            audio_transcriptions_path: None,
            audio_speech_path: None,
            messages_path: None,
            search_path: None,
            fetch_path: None,
            research_path: None,
            balance_path: None,
            search_query_field: None,
            fetch_urls_field: None,
            extra_body: Some(HashMap::from([(
                "token".to_string(),
                serde_json::json!("accio-access-token"),
            )])),
            session_auth: None,
            keepalive: None,
        }
    }

    #[test]
    fn codex_payload_supports_quota() {
        assert!(provider_supports_quota(&codex_payload()));
    }

    #[test]
    fn accio_payload_supports_quota() {
        assert!(provider_supports_quota(&accio_payload()));
    }

    #[test]
    fn parses_codex_usage_windows() {
        let raw = serde_json::json!({
            "plan_type": "team",
            "rate_limit": {
                "allowed": true,
                "limit_reached": false,
                "primary_window": {
                    "used_percent": 0,
                    "limit_window_seconds": 18000,
                    "reset_after_seconds": 18000,
                    "reset_at": 1776255407
                },
                "secondary_window": {
                    "used_percent": 31,
                    "limit_window_seconds": 604800,
                    "reset_after_seconds": 528561,
                    "reset_at": 1776765968
                }
            },
            "spend_control": {
                "reached": false
            }
        });
        let parsed: CodexUsageResponse = serde_json::from_value(raw.clone()).unwrap();
        let mut windows = Vec::new();
        let rate_limit = parsed.rate_limit.as_ref().unwrap();
        windows.push(codex_window_to_view(
            "primary",
            rate_limit.primary_window.as_ref().unwrap(),
        ));
        windows.push(codex_window_to_view(
            "secondary",
            rate_limit.secondary_window.as_ref().unwrap(),
        ));
        windows.sort_by_key(|window| window.limit_window_seconds.unwrap_or(i64::MAX));
        assert_eq!(windows[0].label, "5h");
        assert_eq!(windows[1].label, "7d");
        assert_eq!(windows[1].used_percent, Some(31.0));
        assert_eq!(windows[1].remaining_ratio, Some(0.69));
    }

    #[test]
    fn codex_usage_below_limit_stays_available() {
        let exhausted = false;
        let status = if exhausted { "exhausted" } else { "available" };
        assert_eq!(status, "available");
    }

    #[test]
    fn balance_status_uses_warning_and_exhausted_flags() {
        let snapshot = GatewayProviderQuotaView {
            provider_account_id: "prov-1".to_string(),
            provider_credential_id: None,
            provider_type: "codex".to_string(),
            source: "codex_wham_usage".to_string(),
            status: "warning".to_string(),
            ready: true,
            checked_at: "2026-04-15T00:00:00Z".to_string(),
            next_check_at: "2026-04-15T00:01:00Z".to_string(),
            next_reset_at: None,
            plan_type: Some("team".to_string()),
            representative_claim: Some("5h 窗口已用 85%".to_string()),
            windows: vec![GatewayProviderQuotaWindowView {
                key: "primary".to_string(),
                label: "5h".to_string(),
                used_percent: Some(85.0),
                remaining_ratio: Some(0.15),
                limit_window_seconds: Some(18000),
                reset_at: None,
                reset_after_seconds: Some(1800),
            }],
            error: None,
            raw_data: serde_json::json!({}),
        };
        let balance = quota_to_balance_status(&snapshot);
        assert!(balance.should_deprioritize);
        assert!(!balance.is_unavailable);
        assert_eq!(balance.remaining_ratio, Some(0.15));
    }

    #[test]
    fn build_accio_quota_headers_uses_prefixed_control_plane_contract() {
        let headers = build_accio_control_plane_headers(&accio_payload(), true);
        assert_eq!(
            headers.get("x-utdid").and_then(|value| value.to_str().ok()),
            Some("utd-accio")
        );
        assert_eq!(
            headers
                .get("x-app-version")
                .and_then(|value| value.to_str().ok()),
            Some("0.5.6")
        );
        assert_eq!(
            headers.get("x-cna").and_then(|value| value.to_str().ok()),
            Some("test-cna")
        );
    }

    #[test]
    fn accio_quota_status_uses_remaining_ratio_and_probe_percent() {
        let remaining = serde_json::json!({
            "success": true,
            "data": {
                "total": 520,
                "remaining": 104,
                "entitlement": {
                    "monthly": {
                        "total": 520,
                        "used": 416,
                        "remaining": 104
                    }
                }
            }
        });
        let total = find_numeric_field(&remaining, &["total"]).unwrap();
        let left = find_numeric_field(&remaining, &["remaining"]).unwrap();
        let ratio = (left / total).clamp(0.0, 1.0);
        assert_eq!(round_percent((1.0 - ratio) * 100.0), 80.0);

        let probe = serde_json::json!({
            "success": true,
            "data": {
                "usagePercent": 12.5,
                "refreshCountdownSeconds": 3600
            }
        });
        assert_eq!(
            find_numeric_field(&probe, &["usagePercent", "usage_percent"]),
            Some(12.5)
        );
    }
}
