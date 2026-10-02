use super::http_contract::{build_absolute_url, provider_quota_http_error};
use super::payload_fields::{find_bool_field, find_numeric_field, find_status_string};
use super::refresh_clock::{derive_next_check_at, format_rfc3339};
use super::GatewayProviderQuotaView;
use crate::error::GatewayError;
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::headers::build_upstream_headers;
use rquest::Method;
use serde_json::Value;
use std::time::Duration;
use time::OffsetDateTime;
pub(super) async fn fetch_generic_balance_snapshot(
    timeout_secs: u64,
    provider_account_id: &str,
    provider_credential_id: Option<&str>,
    payload: &ProviderAccountPayload,
) -> Result<GatewayProviderQuotaView, GatewayError> {
    let path = payload
        .balance_path
        .as_deref()
        .ok_or_else(|| GatewayError::bad_request("当前 provider 未配置 balancePath"))?;
    let client = crate::http_client::builder()
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
            let error = error.without_uri();
            GatewayError::service_unavailable(format!("provider balance request failed: {error}"))
                .with_code("provider_quota_request_failed")
        })?;
    let status = response.status();
    let body = response.bytes().await.map_err(|error| {
        let error = error.without_uri();
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
