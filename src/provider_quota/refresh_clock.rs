use super::GatewayProviderQuotaView;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
pub(super) fn quota_refresh_due(snapshot: &GatewayProviderQuotaView) -> bool {
    let Ok(next_check_at) = OffsetDateTime::parse(&snapshot.next_check_at, &Rfc3339) else {
        return true;
    };
    OffsetDateTime::now_utc() >= next_check_at
}

pub(super) fn derive_next_check_at(
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

pub(super) fn format_rfc3339(value: OffsetDateTime) -> String {
    value
        .format(&Rfc3339)
        .unwrap_or_else(|_| "9999-12-31T23:59:59Z".to_string())
}
