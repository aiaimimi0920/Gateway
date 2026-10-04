//! Normalize only supported wait headers; never retain or log the header map.
use std::time::SystemTime;

use rquest::header::HeaderMap;

const MAX_HINT_VALUE_BYTES: usize = 128;

mod body;
pub(super) use body::parse as parse_body;

/// Millisecond headers take precedence, then the standard seconds/HTTP-date header.
/// Overflow/oversize is a terminally large hint, not a shorter fallback delay.
pub(crate) fn retry_after_from_headers(headers: &HeaderMap, now: SystemTime) -> Option<u64> {
    for (name, multiplier) in [
        ("retry-after-ms", 1),
        ("x-ms-retry-after-ms", 1),
        ("retry-after", 1_000),
    ] {
        let mut values = headers.get_all(name).iter();
        let Some(value) = values.next() else {
            continue;
        };
        // These are singleton headers. Do not pick an arbitrary duplicate value.
        if values.next().is_some() {
            continue;
        }
        let Ok(value) = value.to_str() else {
            continue;
        };
        let value = value.trim();
        if value.len() > MAX_HINT_VALUE_BYTES {
            return Some(u64::MAX);
        }
        if let Some(delay) = integer_delay(value, multiplier) {
            return Some(delay);
        }
        if name == "retry-after" {
            if let Ok(date) = httpdate::parse_http_date(value) {
                let delay = date.duration_since(now).unwrap_or_default();
                // Round up: truncating a sub-millisecond remainder can send early.
                let millis = delay.as_millis() + u128::from(delay.subsec_nanos() % 1_000_000 != 0);
                return Some(u64::try_from(millis).unwrap_or(u64::MAX));
            }
        }
    }
    None
}

fn integer_delay(value: &str, multiplier: u64) -> Option<u64> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    Some(
        value
            .parse::<u64>()
            .ok()
            .and_then(|value| value.checked_mul(multiplier))
            .unwrap_or(u64::MAX),
    )
}

#[cfg(test)]
mod tests;
