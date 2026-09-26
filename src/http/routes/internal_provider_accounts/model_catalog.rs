//! Catalog model-name normalization, aliases and provider allow/exclude constraints.

use crate::routing::candidate::ProviderAccountPayload;
use serde_json::Value;
use std::collections::BTreeMap;

pub(super) fn extract_model_names_from_value(value: &Value) -> Vec<String> {
    let mut names = Vec::new();
    if let Some(array) = value.as_array() {
        for item in array {
            push_model_name(item, &mut names);
        }
    }
    for key in ["data", "models", "items", "modelList", "model_list"] {
        if let Some(array) = value.get(key).and_then(Value::as_array) {
            for item in array {
                push_model_name(item, &mut names);
            }
        }
    }
    if let Some(result) = value.get("result") {
        names.extend(extract_model_names_from_value(result));
    }
    normalize_model_names(names)
}

fn push_model_name(value: &Value, names: &mut Vec<String>) {
    if let Some(model) = value.as_str() {
        names.push(model.to_string());
        return;
    }
    if let Some(object) = value.as_object() {
        for key in ["id", "model", "name", "modelName"] {
            if let Some(model) = object.get(key).and_then(Value::as_str) {
                names.push(model.to_string());
                return;
            }
        }
    }
}

pub(super) fn extract_accio_catalog_model_names(value: &Value) -> Vec<String> {
    let mut names = Vec::new();
    if let Some(data) = value.get("data").and_then(Value::as_array) {
        for provider in data {
            if let Some(model_list) = provider.get("modelList").and_then(Value::as_array) {
                for item in model_list {
                    push_model_name(item, &mut names);
                }
            }
        }
    }
    if names.is_empty() {
        return extract_model_names_from_value(value);
    }
    normalize_model_names(names)
}

pub(super) fn read_configured_supported_models(payload: &Value) -> Vec<String> {
    let values = payload
        .get("supportedModels")
        .or_else(|| payload.get("supported_models"))
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    normalize_model_names(values)
}

fn read_allowed_models(payload: &Value) -> Vec<String> {
    let values = payload
        .get("allowedModels")
        .or_else(|| payload.get("allowed_models"))
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    normalize_model_names(values)
}

fn read_excluded_models(payload: &Value) -> Vec<String> {
    let mut values = payload
        .get("excludedModels")
        .or_else(|| payload.get("excluded_models"))
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if let Some(disabled) = payload
        .get("disabledModels")
        .or_else(|| payload.get("disabled_models"))
        .and_then(Value::as_object)
    {
        values.extend(disabled.keys().cloned());
    }
    normalize_model_names(values)
}

pub(super) fn filter_model_names_by_payload_constraints(
    payload: &Value,
    models: Vec<String>,
) -> Vec<String> {
    let supported = read_configured_supported_models(payload);
    let allowed = read_allowed_models(payload);
    let excluded = read_excluded_models(payload)
        .into_iter()
        .map(|value| value.to_ascii_lowercase())
        .collect::<std::collections::HashSet<_>>();

    let supported_set = (!supported.is_empty()).then(|| {
        supported
            .into_iter()
            .map(|value| value.to_ascii_lowercase())
            .collect::<std::collections::HashSet<_>>()
    });
    let allowed_set = (!allowed.is_empty()).then(|| {
        allowed
            .into_iter()
            .map(|value| value.to_ascii_lowercase())
            .collect::<std::collections::HashSet<_>>()
    });

    normalize_model_names(
        models
            .into_iter()
            .filter(|model| {
                let key = model.to_ascii_lowercase();
                if excluded.contains(&key) {
                    return false;
                }
                if let Some(supported_set) = supported_set.as_ref() {
                    if !supported_set.contains(&key) {
                        return false;
                    }
                }
                if let Some(allowed_set) = allowed_set.as_ref() {
                    if !allowed_set.contains(&key) {
                        return false;
                    }
                }
                true
            })
            .collect(),
    )
}

pub(super) fn read_default_model_from_payload(
    raw_payload: &Value,
    payload: &ProviderAccountPayload,
) -> Option<String> {
    raw_payload
        .get("defaultModel")
        .or_else(|| raw_payload.get("default_model"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| payload.default_model.clone())
}

pub(super) fn provider_model_key(model_code: &str, upstream_model: Option<&str>) -> String {
    upstream_model
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| model_code.trim())
        .to_string()
}

pub(super) fn normalize_model_names(models: Vec<String>) -> Vec<String> {
    let mut map = BTreeMap::<String, String>::new();
    for model in models {
        let trimmed = model.trim();
        if trimmed.is_empty() {
            continue;
        }
        let key = trimmed.to_ascii_lowercase();
        map.entry(key).or_insert_with(|| trimmed.to_string());
    }
    map.into_values().collect()
}
