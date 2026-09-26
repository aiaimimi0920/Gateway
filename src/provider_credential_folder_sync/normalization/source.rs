//! Read import-source fields and retain the existing bounded rawSource traversal.

use serde_json::Value;

pub(super) fn canonicalize_folder_sync_raw_source(raw_payload: &Value) -> Value {
    let mut cursor = raw_payload;
    let mut depth = 0usize;
    while depth < 32 {
        let Some(raw_map) = cursor.as_object() else {
            break;
        };
        let Some(next) = raw_map.get("rawSource") else {
            break;
        };
        cursor = next;
        depth += 1;
    }

    match cursor {
        Value::Object(raw_map) => {
            let mut sanitized = raw_map.clone();
            sanitized.remove("rawSource");
            Value::Object(sanitized)
        }
        other => other.clone(),
    }
}

pub(super) fn read_optional_object_string(
    raw_map: &serde_json::Map<String, Value>,
    keys: &[&str],
) -> Option<String> {
    keys.iter().find_map(|key| {
        raw_map
            .get(*key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    })
}

pub(super) fn read_optional_object_string_array(
    raw_map: &serde_json::Map<String, Value>,
    keys: &[&str],
) -> Option<Vec<String>> {
    keys.iter().find_map(|key| {
        let values = raw_map
            .get(*key)
            .and_then(Value::as_array)?
            .iter()
            .filter_map(|entry| {
                entry
                    .as_str()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_string)
            })
            .collect::<Vec<_>>();
        (!values.is_empty()).then_some(values)
    })
}

pub(super) fn extract_cookie_value<'a>(
    cookie_header: &'a str,
    cookie_name: &str,
) -> Option<&'a str> {
    cookie_header
        .split(';')
        .filter_map(|segment| segment.split_once('='))
        .find_map(|(name, value)| {
            if name.trim().eq_ignore_ascii_case(cookie_name) {
                let trimmed = value.trim();
                (!trimmed.is_empty()).then_some(trimmed)
            } else {
                None
            }
        })
}
