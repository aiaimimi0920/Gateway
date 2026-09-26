use super::http_contract::provider_quota_http_error;
use super::payload_fields::round_percent;
use super::refresh_clock::{derive_next_check_at, format_rfc3339};
use super::{
    CodexUsageResponse, CodexUsageWindow, GatewayProviderQuotaView, GatewayProviderQuotaWindowView,
};
use crate::error::GatewayError;
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::headers::build_upstream_headers;
use rquest::Method;
use serde_json::Value;
use std::time::Duration;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
pub(super) async fn fetch_codex_quota_snapshot(
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
            let error = error.without_url();
            GatewayError::service_unavailable(format!("Codex quota request failed: {error}"))
                .with_code("provider_quota_request_failed")
        })?;
    let status = response.status();
    let body = response.bytes().await.map_err(|error| {
        let error = error.without_url();
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

pub(super) fn codex_window_to_view(
    key: &str,
    window: &CodexUsageWindow,
) -> GatewayProviderQuotaWindowView {
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

fn window_label_from_seconds(limit_window_seconds: Option<i64>) -> Option<String> {
    match limit_window_seconds.unwrap_or_default() {
        18_000 => Some("5h".to_string()),
        86_400 => Some("1d".to_string()),
        604_800 => Some("7d".to_string()),
        seconds if seconds > 0 => Some(format!("{}s", seconds)),
        _ => None,
    }
}
