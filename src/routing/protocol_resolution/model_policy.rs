//! Model-scoped protocol allow/block policy and ordered family parsing.

use super::surfaces::surface_supported_wire_protocol_families;
use crate::protocol::registry::canonicalize_wire_protocol_family_key;
use serde_json::Value;

pub fn resolve_supported_wire_protocol_families_for_model(
    payload: Option<&Value>,
    model_alias: Option<&str>,
    upstream_model: Option<&str>,
    adapter: &str,
    fallback_protocol_family: &str,
) -> Vec<String> {
    let surface_supported =
        surface_supported_wire_protocol_families(adapter, fallback_protocol_family);
    let Some(payload) = payload else {
        return surface_supported;
    };

    let model_candidates = collect_model_candidates(model_alias, upstream_model);
    let model_allowed = read_model_scoped_family_values(
        payload,
        &model_candidates,
        &[
            "protocolFamiliesByModel",
            "protocol_families_by_model",
            "supportedProtocolFamiliesByModel",
            "supported_protocol_families_by_model",
            "allowedProtocolFamiliesByModel",
            "allowed_protocol_families_by_model",
        ],
        &[
            "protocolFamilies",
            "protocol_families",
            "supportedProtocolFamilies",
            "supported_protocol_families",
            "allowedProtocolFamilies",
            "allowed_protocol_families",
            "families",
        ],
    );
    let global_allowed = read_family_values_from_object(
        payload,
        &[
            "protocolFamilies",
            "protocol_families",
            "supportedProtocolFamilies",
            "supported_protocol_families",
            "allowedProtocolFamilies",
            "allowed_protocol_families",
            "protocolFamily",
            "protocol_family",
        ],
    );
    let mut allowed = if !model_allowed.is_empty() {
        model_allowed
    } else if !global_allowed.is_empty() {
        global_allowed
    } else {
        surface_supported.clone()
    };
    if allowed.is_empty() {
        allowed = surface_supported.clone();
    }

    let model_blocked = read_model_scoped_family_values(
        payload,
        &model_candidates,
        &[
            "excludedProtocolFamiliesByModel",
            "excluded_protocol_families_by_model",
            "blockedProtocolFamiliesByModel",
            "blocked_protocol_families_by_model",
        ],
        &[
            "excludedProtocolFamilies",
            "excluded_protocol_families",
            "blockedProtocolFamilies",
            "blocked_protocol_families",
            "families",
        ],
    );
    let global_blocked = read_family_values_from_object(
        payload,
        &[
            "excludedProtocolFamilies",
            "excluded_protocol_families",
            "blockedProtocolFamilies",
            "blocked_protocol_families",
        ],
    );

    let mut resolved = Vec::new();
    for family in allowed {
        let canonical = canonicalize_wire_protocol_family_key(&family);
        if !surface_supported.iter().any(|value| value == &canonical) {
            continue;
        }
        if global_blocked
            .iter()
            .chain(model_blocked.iter())
            .any(|blocked| blocked == &canonical)
        {
            continue;
        }
        push_unique(&mut resolved, canonical);
    }
    resolved
}

fn collect_model_candidates(
    model_alias: Option<&str>,
    upstream_model: Option<&str>,
) -> Vec<String> {
    let mut values = Vec::new();
    if let Some(value) = model_alias.and_then(normalize_model_hint) {
        push_unique(&mut values, value);
    }
    if let Some(value) = upstream_model.and_then(normalize_model_hint) {
        push_unique(&mut values, value);
    }
    values
}

fn normalize_model_hint(value: &str) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then_some(trimmed.to_ascii_lowercase())
}

fn model_key_matches(pattern: &str, candidates: &[String]) -> bool {
    let normalized = canonicalize_model_pattern(pattern);
    if normalized.is_empty() {
        return false;
    }
    candidates.iter().any(|candidate| {
        if let Some(prefix) = normalized.strip_suffix('*') {
            candidate.starts_with(prefix)
        } else {
            candidate == &normalized
        }
    })
}

fn canonicalize_model_pattern(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

fn read_family_values_from_object(payload: &Value, keys: &[&str]) -> Vec<String> {
    let Some(object) = payload.as_object() else {
        return Vec::new();
    };
    let mut values = Vec::new();
    for key in keys {
        if let Some(value) = object.get(*key) {
            collect_family_values(value, &mut values);
        }
    }
    values
}

fn read_model_scoped_family_values(
    payload: &Value,
    model_candidates: &[String],
    top_level_keys: &[&str],
    nested_keys: &[&str],
) -> Vec<String> {
    let Some(object) = payload.as_object() else {
        return Vec::new();
    };
    let mut values = Vec::new();
    for key in top_level_keys {
        let Some(Value::Object(model_map)) = object.get(*key) else {
            continue;
        };
        for (model_key, value) in model_map {
            if !model_key_matches(model_key, model_candidates) {
                continue;
            }
            match value {
                Value::Object(nested) => {
                    for nested_key in nested_keys {
                        if let Some(nested_value) = nested.get(*nested_key) {
                            collect_family_values(nested_value, &mut values);
                        }
                    }
                }
                _ => collect_family_values(value, &mut values),
            }
        }
    }
    values
}

fn collect_family_values(value: &Value, output: &mut Vec<String>) {
    match value {
        Value::String(text) => {
            for item in text
                .split([',', '\n'])
                .map(str::trim)
                .filter(|item| !item.is_empty())
            {
                push_unique(output, canonicalize_wire_protocol_family_key(item));
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_family_values(item, output);
            }
        }
        _ => {}
    }
}

fn push_unique(values: &mut Vec<String>, value: String) {
    if !values.iter().any(|existing| existing == &value) {
        values.push(value);
    }
}
