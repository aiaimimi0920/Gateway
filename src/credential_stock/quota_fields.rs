use crate::provider_quota::GatewayProviderQuotaView;
use serde_json::Value;

pub(super) fn remaining_tokens_for_policy_window(
    snapshot: &GatewayProviderQuotaView,
) -> Option<i64> {
    find_numeric_field(
        &snapshot.raw_data,
        &[
            "remainingTokens",
            "remaining_tokens",
            "availableTokens",
            "available_tokens",
            "remaining",
        ],
    )
    .or_else(|| {
        snapshot
            .windows
            .iter()
            .filter_map(|window| {
                let ratio = window.remaining_ratio?;
                let limit = find_numeric_field(
                    &snapshot.raw_data,
                    &[
                        "tokenLimit",
                        "token_limit",
                        "totalTokens",
                        "total_tokens",
                        "limit",
                    ],
                )?;
                Some((ratio * limit as f64).round() as i64)
            })
            .min()
    })
}

fn find_numeric_field(value: &Value, keys: &[&str]) -> Option<i64> {
    match value {
        Value::Object(map) => {
            for key in keys {
                if let Some(value) = map.get(*key).and_then(value_to_i64) {
                    return Some(value);
                }
            }
            map.values()
                .find_map(|nested| find_numeric_field(nested, keys))
        }
        Value::Array(values) => values
            .iter()
            .find_map(|nested| find_numeric_field(nested, keys)),
        _ => None,
    }
}

fn value_to_i64(value: &Value) -> Option<i64> {
    value
        .as_i64()
        .or_else(|| value.as_u64().and_then(|value| i64::try_from(value).ok()))
        .or_else(|| value.as_f64().map(|value| value.round() as i64))
}
