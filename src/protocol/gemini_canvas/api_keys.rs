use regex::Regex;
use serde_json::Value;
use std::sync::OnceLock;

use crate::routing::candidate::ProviderAccountPayload;

pub fn direct_http_google_api_key(
    payload: &ProviderAccountPayload,
    storage_state: &Value,
) -> Option<String> {
    payload
        .extra_body
        .as_ref()
        .and_then(|extra_body| {
            read_optional_hash_string(
                extra_body,
                &["googleApiKey", "google_api_key", "apiKey", "api_key"],
            )
        })
        .or_else(|| {
            extract_google_api_keys_from_value(storage_state)
                .into_iter()
                .next()
        })
        .or_else(|| {
            payload.extra_body.as_ref().and_then(|extra_body| {
                read_optional_hash_string_array_values(extra_body)
                    .into_iter()
                    .next()
            })
        })
        .or_else(|| {
            let api_key = payload.api_key.trim();
            if api_key.is_empty() {
                None
            } else {
                Some(api_key.to_string())
            }
        })
}

pub fn direct_http_google_api_keys(
    payload: &ProviderAccountPayload,
    storage_state: &Value,
) -> Vec<String> {
    let mut keys = Vec::new();
    let has_explicit_extra_primary = payload.extra_body.as_ref().is_some_and(|extra_body| {
        read_optional_hash_string(
            extra_body,
            &["googleApiKey", "google_api_key", "apiKey", "api_key"],
        )
        .is_some()
    });
    if !has_explicit_extra_primary && !payload.api_key.trim().is_empty() {
        extend_unique_strings(&mut keys, vec![payload.api_key.trim().to_string()]);
    }
    if let Some(extra_body) = payload.extra_body.as_ref() {
        extend_unique_strings(&mut keys, extract_google_api_keys_from_hash_map(extra_body));
    }
    extend_unique_strings(&mut keys, extract_google_api_keys_from_value(storage_state));
    keys
}

pub fn extract_google_api_keys_from_page_blob(blob: &str) -> Vec<String> {
    static GOOGLE_API_KEY_REGEX: OnceLock<Regex> = OnceLock::new();
    let regex = GOOGLE_API_KEY_REGEX.get_or_init(|| {
        Regex::new(r"AIza[0-9A-Za-z\-_]{20,}").expect("google api key regex must compile")
    });
    let mut keys = Vec::new();
    for capture in regex.find_iter(blob) {
        let candidate = capture.as_str().trim();
        if candidate.is_empty() || keys.iter().any(|existing| existing == candidate) {
            continue;
        }
        keys.push(candidate.to_string());
    }
    keys
}

fn read_optional_string(value: &Value, keys: &[&str]) -> Option<String> {
    let map = value.as_object()?;
    for key in keys {
        if let Some(value) = map.get(*key).and_then(|entry| entry.as_str()) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}

pub(super) fn read_optional_hash_string(
    map: &std::collections::HashMap<String, Value>,
    keys: &[&str],
) -> Option<String> {
    for key in keys {
        if let Some(value) = map.get(*key).and_then(|entry| entry.as_str()) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}

fn collect_non_empty_strings_from_slice(values: &[Value]) -> Vec<String> {
    let mut collected = Vec::new();
    for entry in values {
        let Some(candidate) = entry
            .as_str()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        if collected.iter().any(|existing| existing == candidate) {
            continue;
        }
        collected.push(candidate.to_string());
    }
    collected
}

fn read_optional_hash_string_array_values(
    map: &std::collections::HashMap<String, Value>,
) -> Vec<String> {
    ["apiKeys", "api_keys"]
        .iter()
        .find_map(|key| {
            map.get(*key)
                .and_then(Value::as_array)
                .map(|values| collect_non_empty_strings_from_slice(values))
        })
        .unwrap_or_default()
}

fn read_optional_string_array_values(value: &Value, keys: &[&str]) -> Vec<String> {
    let Some(map) = value.as_object() else {
        return Vec::new();
    };
    keys.iter()
        .find_map(|key| {
            map.get(*key)
                .and_then(Value::as_array)
                .map(|values| collect_non_empty_strings_from_slice(values))
        })
        .unwrap_or_default()
}

fn extend_unique_strings(target: &mut Vec<String>, values: Vec<String>) {
    for candidate in values {
        if target.iter().any(|existing| existing == &candidate) {
            continue;
        }
        target.push(candidate);
    }
}

fn extract_google_api_keys_from_hash_map(
    map: &std::collections::HashMap<String, Value>,
) -> Vec<String> {
    let mut keys = Vec::new();
    if let Some(primary) = read_optional_hash_string(
        map,
        &["googleApiKey", "google_api_key", "apiKey", "api_key"],
    ) {
        keys.push(primary);
    }
    extend_unique_strings(&mut keys, read_optional_hash_string_array_values(map));
    keys
}

fn extract_google_api_keys_from_value(value: &Value) -> Vec<String> {
    let mut keys = Vec::new();
    if let Some(primary) = read_optional_string(
        value,
        &["googleApiKey", "google_api_key", "apiKey", "api_key"],
    ) {
        keys.push(primary);
    }
    extend_unique_strings(
        &mut keys,
        read_optional_string_array_values(value, &["apiKeys", "api_keys"]),
    );
    if let Some(config) = value.as_object().and_then(|map| {
        map.get("firebaseConfig")
            .or_else(|| map.get("firebase_config"))
    }) {
        if let Some(primary) = read_optional_string(config, &["apiKey", "api_key"]) {
            extend_unique_strings(&mut keys, vec![primary]);
        }
        extend_unique_strings(
            &mut keys,
            read_optional_string_array_values(config, &["apiKeys", "api_keys"]),
        );
    }
    keys
}
