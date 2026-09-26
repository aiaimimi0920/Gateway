use super::GatewayProviderQuotaView;
use crate::balance::BalanceStatus;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
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
