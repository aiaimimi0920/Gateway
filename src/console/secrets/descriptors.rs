//! Field removal and secret descriptors preserve deterministic paths and bounded previews.

use super::classification::is_sensitive_key;
use super::SecretDescriptor;
use crate::console::document::encode_pointer_segment;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, VecDeque};

pub(super) fn redact_string_map(
    map: &mut HashMap<String, String>,
    base_path: &str,
    descriptors: &mut Vec<SecretDescriptor>,
) {
    let mut keys: Vec<String> = map
        .keys()
        .filter(|key| is_sensitive_key(key))
        .cloned()
        .collect();
    keys.sort();
    for key in keys {
        if let Some(value) = map.remove(&key) {
            push_value_descriptor(
                descriptors,
                format!("{base_path}/{}", encode_pointer_segment(&key)),
                &Value::String(value),
                true,
            );
        }
    }
}

pub(super) fn redact_extra_body_map(
    map: &mut HashMap<String, Value>,
    base_path: &str,
    descriptors: &mut Vec<SecretDescriptor>,
) {
    let mut keys: Vec<String> = map.keys().cloned().collect();
    keys.sort();
    for key in keys {
        let path = format!("{base_path}/{}", encode_pointer_segment(&key));
        if is_sensitive_key(&key) {
            if let Some(value) = map.remove(&key) {
                push_value_descriptor(descriptors, path, &value, true);
            }
        } else if let Some(value) = map.get_mut(&key) {
            redact_json_value(value, &path, descriptors);
        }
    }
}

fn redact_json_value(value: &mut Value, base_path: &str, descriptors: &mut Vec<SecretDescriptor>) {
    match value {
        Value::Object(map) => {
            let mut keys: Vec<String> = map.keys().cloned().collect();
            keys.sort();
            for key in keys {
                let path = format!("{base_path}/{}", encode_pointer_segment(&key));
                if is_sensitive_key(&key) {
                    if let Some(value) = map.remove(&key) {
                        push_value_descriptor(descriptors, path, &value, true);
                    }
                } else if let Some(value) = map.get_mut(&key) {
                    redact_json_value(value, &path, descriptors);
                }
            }
        }
        Value::Array(values) => {
            for (index, value) in values.iter_mut().enumerate() {
                redact_json_value(value, &format!("{base_path}/{index}"), descriptors);
            }
        }
        _ => {}
    }
}

pub(super) fn push_string_descriptor(
    descriptors: &mut Vec<SecretDescriptor>,
    path: String,
    value: &str,
    configured: bool,
) {
    push_value_descriptor(
        descriptors,
        path,
        &Value::String(value.to_string()),
        configured,
    );
}

pub(super) fn push_optional_string_descriptor(
    descriptors: &mut Vec<SecretDescriptor>,
    path: String,
    value: Option<&str>,
) {
    match value {
        Some(value) => push_string_descriptor(descriptors, path, value, true),
        None => descriptors.push(SecretDescriptor {
            path,
            configured: false,
            fingerprint: None,
            preview: None,
        }),
    }
}

fn push_value_descriptor(
    descriptors: &mut Vec<SecretDescriptor>,
    path: String,
    value: &Value,
    configured: bool,
) {
    let (fingerprint, preview) = if configured {
        (Some(fingerprint_value(value)), preview_value(value))
    } else {
        (None, None)
    };
    descriptors.push(SecretDescriptor {
        path,
        configured,
        fingerprint,
        preview,
    });
}

fn fingerprint_value(value: &Value) -> String {
    let encoded = serde_json::to_vec(value).unwrap_or_default();
    let mut hasher = Sha256::new();
    hasher.update(encoded);
    format!("sha256:{}", &hex::encode(hasher.finalize())[..12])
}

fn preview_value(value: &Value) -> Option<String> {
    let value = value.as_str()?;
    let mut prefix = String::new();
    let mut suffix = VecDeque::with_capacity(3);
    let mut count = 0usize;
    for character in value.chars() {
        count += 1;
        if count <= 3 {
            prefix.push(character);
        }
        if suffix.len() == 3 {
            suffix.pop_front();
        }
        suffix.push_back(character);
    }
    if count == 0 {
        return None;
    }
    if count <= 8 {
        return Some("***".to_string());
    }
    let suffix: String = suffix.into_iter().collect();
    Some(format!("{prefix}***{suffix}"))
}
