//! UTC usage-report timestamps from monotonic and wall-clock values.

/// Format an `Instant` as an approximate ISO-8601 UTC string by computing
/// the offset from the current time.
pub(super) fn format_instant_as_iso(started_at: &std::time::Instant) -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let elapsed = started_at.elapsed();
    let now_secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let started_secs = now_secs.saturating_sub(elapsed.as_secs());
    format_unix_secs_as_iso(started_secs)
}

pub(super) fn format_now_as_iso() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format_unix_secs_as_iso(secs)
}

pub(super) fn format_unix_secs_as_iso(secs: u64) -> String {
    // Minimal UNIX → ISO-8601 converter (no external deps).
    let mut remaining = secs;
    let s = remaining % 60;
    remaining /= 60;
    let mi = remaining % 60;
    remaining /= 60;
    let h = remaining % 24;
    let mut days = (remaining / 24) as i64;

    days += 719_468;
    let era = if days >= 0 { days } else { days - 146_096 } / 146_097;
    let doe = days - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if month <= 2 { y + 1 } else { y };

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        year, month, d, h, mi, s
    )
}
