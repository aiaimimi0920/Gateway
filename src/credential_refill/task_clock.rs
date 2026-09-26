use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

pub(super) fn now_rfc3339() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| "unknown".to_string())
}

pub(super) fn future_rfc3339(seconds: u64) -> String {
    let seconds = i64::try_from(seconds).unwrap_or(i64::MAX);
    OffsetDateTime::now_utc()
        .saturating_add(time::Duration::seconds(seconds))
        .format(&Rfc3339)
        .unwrap_or_else(|_| "unknown".to_string())
}
