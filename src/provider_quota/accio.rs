use super::accio_http::fetch_accio_quota_endpoint;
use super::payload_fields::{find_numeric_field, round_percent};
use super::refresh_clock::{derive_next_check_at, format_rfc3339};
use super::{GatewayProviderQuotaView, GatewayProviderQuotaWindowView};
use crate::error::GatewayError;
use crate::implementation_lines;
use crate::routing::candidate::ProviderAccountPayload;
use time::OffsetDateTime;
pub(super) async fn fetch_accio_quota_snapshot(
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
