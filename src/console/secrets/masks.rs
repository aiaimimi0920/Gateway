//! Mask sentinels and generated previews can never become replacement secrets.

use serde_json::Value;
use std::collections::HashSet;

pub(super) fn contains_mask_sentinel(value: &Value, previews: &HashSet<String>) -> bool {
    match value {
        Value::String(value) => is_basic_mask_sentinel(value) || previews.contains(value),
        Value::Array(values) => values
            .iter()
            .any(|value| contains_mask_sentinel(value, previews)),
        Value::Object(map) => map
            .values()
            .any(|value| contains_mask_sentinel(value, previews)),
        _ => false,
    }
}

pub(super) fn contains_basic_mask_sentinel(value: &Value) -> bool {
    match value {
        Value::String(value) => is_basic_mask_sentinel(value),
        Value::Array(values) => values.iter().any(contains_basic_mask_sentinel),
        Value::Object(map) => map.values().any(contains_basic_mask_sentinel),
        _ => false,
    }
}

fn is_basic_mask_sentinel(value: &str) -> bool {
    let normalized = value.trim().to_ascii_lowercase();
    if matches!(
        normalized.as_str(),
        "***" | "..." | "…" | "[redacted]" | "<redacted>"
    ) || normalized.contains("***")
    {
        return true;
    }

    let has_ellipsis = normalized.contains("...") || normalized.contains('…');
    has_ellipsis
        && [
            "sk-", "pk-", "rk-", "sess-", "token-", "token ", "bearer ", "secret-", "secret ",
            "api-key", "apikey",
        ]
        .iter()
        .any(|prefix| normalized.starts_with(prefix))
}
