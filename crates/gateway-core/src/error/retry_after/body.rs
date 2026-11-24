//! Borrow validated JSON tokens and round decimal seconds without float loss.
use serde::Deserialize;
use serde_json::value::RawValue;

#[derive(Deserialize)]
struct Hints<'a> {
    #[serde(default, borrow, deserialize_with = "present")]
    retry_after: Option<&'a RawValue>,
    #[serde(default, borrow, rename = "retryAfter", deserialize_with = "present")]
    retry_after_camel: Option<&'a RawValue>,
    #[serde(borrow)]
    error: Option<&'a RawValue>,
}

fn present<'de, D>(deserializer: D) -> Result<Option<&'de RawValue>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    // A present null/invalid value keeps its priority rather than selecting a
    // later field. This preserves the existing body-field selection contract.
    <&RawValue>::deserialize(deserializer).map(Some)
}

pub(in crate::error) fn parse(body: &str) -> Option<u64> {
    let hints: Hints<'_> = serde_json::from_str(body).ok()?;
    let nested: Option<Hints<'_>> = hints
        .error
        .and_then(|error| serde_json::from_str(error.get()).ok());
    let candidate = hints
        .retry_after
        .or(hints.retry_after_camel)
        .or_else(|| nested.as_ref().and_then(|hints| hints.retry_after))
        .or_else(|| nested.as_ref().and_then(|hints| hints.retry_after_camel))?;
    decimal_millis(candidate.get())
}

fn decimal_millis(token: &str) -> Option<u64> {
    // RawValue has already validated JSON syntax, but may hold a non-number.
    if !matches!(token.as_bytes().first(), Some(b'0'..=b'9' | b'-')) {
        return None;
    }
    let (negative, token) = token
        .strip_prefix('-')
        .map_or((false, token), |rest| (true, rest));
    let (mantissa, exponent) = token.split_once(['e', 'E']).unwrap_or((token, "0"));
    let is_zero = mantissa.bytes().all(|byte| matches!(byte, b'0' | b'.'));
    if is_zero {
        return Some(0);
    }
    if negative {
        return None;
    }
    // Limit numeric work/allocation independently of the bounded HTTP body.
    if token.len() > super::MAX_HINT_VALUE_BYTES {
        return Some(u64::MAX);
    }
    let exponent: i64 = exponent.parse().unwrap_or_else(|_| {
        if exponent.starts_with('-') {
            i64::MIN
        } else {
            i64::MAX
        }
    });
    let fractional = mantissa
        .split_once('.')
        .map_or(0, |(_, digits)| digits.len());
    let digits: String = mantissa.chars().filter(|&char| char != '.').collect();
    let digits = digits.trim_start_matches('0');
    let scale = exponent.saturating_add(3).saturating_sub(fractional as i64);
    if scale >= 0 {
        if scale > 19 || digits.len().saturating_add(scale as usize) > 20 {
            return Some(u64::MAX);
        }
        return Some(
            digits
                .parse::<u64>()
                .ok()
                .and_then(|value| value.checked_mul(10_u64.pow(scale as u32)))
                .unwrap_or(u64::MAX),
        );
    }
    let discarded = scale.unsigned_abs();
    if discarded >= digits.len() as u64 {
        return Some(1);
    }
    let kept = digits.len() - discarded as usize;
    let floor = digits[..kept].parse::<u64>().ok();
    let round_up = u64::from(digits[kept..].bytes().any(|byte| byte != b'0'));
    Some(
        floor
            .and_then(|value| value.checked_add(round_up))
            .unwrap_or(u64::MAX),
    )
}
