//! Credential storage overrides and public payload/header redaction.

use crate::db;
use serde_json::Value;

pub(super) fn normalize_provider_credential_payload_for_storage(
    provider_account: &db::GatewayProviderAccountView,
    payload: &Value,
) -> Value {
    let mut payload = payload.clone();
    if let Some(object) = payload.as_object_mut() {
        if !object.contains_key("adapter") {
            object.insert(
                "adapter".to_string(),
                Value::String(provider_account.adapter.clone()),
            );
        }
        strip_empty_top_level_override(object, "baseUrl");
        strip_empty_top_level_override(object, "base_url");
        strip_empty_top_level_override(object, "apiKey");
        strip_empty_top_level_override(object, "api_key");
    }
    payload
}

fn strip_empty_top_level_override(object: &mut serde_json::Map<String, Value>, key: &str) {
    let should_remove = object
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .is_some_and(|value| value.is_empty());
    if should_remove {
        object.remove(key);
    }
}

pub(super) fn mask_provider_payload_secrets(value: Value) -> Value {
    match value {
        Value::Object(map) => Value::Object(
            map.into_iter()
                .map(|(key, value)| {
                    let lower = key.to_lowercase();
                    let next = if lower == "headers" {
                        mask_headers(value)
                    } else if is_secret_key(&lower) {
                        Value::String(mask_secret_string(value.as_str().unwrap_or("***")))
                    } else {
                        mask_provider_payload_secrets(value)
                    };
                    (key, next)
                })
                .collect(),
        ),
        Value::Array(items) => Value::Array(
            items
                .into_iter()
                .map(mask_provider_payload_secrets)
                .collect(),
        ),
        other => other,
    }
}

fn mask_headers(value: Value) -> Value {
    match value {
        Value::Object(headers) => Value::Object(
            headers
                .into_iter()
                .map(|(key, value)| {
                    let masked = if is_secret_key(&key.to_lowercase()) {
                        Value::String(mask_secret_string(value.as_str().unwrap_or("***")))
                    } else {
                        value
                    };
                    (key, masked)
                })
                .collect(),
        ),
        other => other,
    }
}

fn is_secret_key(key: &str) -> bool {
    key.contains("auth")
        || key.contains("token")
        || key.contains("secret")
        || key.contains("cookie")
        || key.contains("apikey")
        || key.ends_with("key")
}

fn mask_secret_string(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.len() <= 8 {
        return "***".to_string();
    }
    let (Some(prefix), Some(suffix)) = (trimmed.get(..3), trimmed.get(trimmed.len() - 3..)) else {
        return "***".to_string();
    };
    format!("{prefix}***{suffix}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn secret_preview_rejects_partial_unicode_boundaries() {
        for secret in ["ab\u{00e9}defghij", "abcdefghij\u{00e9}xy"] {
            let masked = mask_provider_payload_secrets(json!({
                "apiKey": secret,
                "headers": {"Authorization": secret, "X-Trace": "preserved"},
                "nested": [{"refreshToken": secret}]
            }));
            assert_eq!(masked["apiKey"], "***");
            assert_eq!(masked["headers"]["Authorization"], "***");
            assert_eq!(masked["headers"]["X-Trace"], "preserved");
            assert_eq!(masked["nested"][0]["refreshToken"], "***");
        }
    }

    #[test]
    fn secret_preview_preserves_ascii_and_complete_unicode_boundaries() {
        for (input, expected) in [
            ("", "***"),
            ("abcdefgh", "***"),
            ("  abcdefghi  ", "abc***ghi"),
            ("a\u{00e9}bcdef\u{00e9}z", "a\u{00e9}***\u{00e9}z"),
        ] {
            assert_eq!(mask_secret_string(input), expected);
        }
        let masked = mask_provider_payload_secrets(json!({"apiKey": 42, "token": null}));
        assert_eq!(masked, json!({"apiKey": "***", "token": "***"}));
    }
}
