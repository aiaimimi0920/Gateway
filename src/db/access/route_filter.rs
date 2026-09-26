use super::bundle_compat::openai_text_endpoint_family;
use super::*;

pub(super) fn route_rows_for_endpoint(
    projection: &CachedAccessProjection,
    model: &str,
    endpoint_kind: EndpointKind,
) -> Vec<ProjectedPlatformAccessRow> {
    let endpoint_kind = endpoint_kind_name(endpoint_kind);
    let rows = projection
        .rows_by_model
        .get(model)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .collect::<Vec<_>>();
    let exact_rows = rows
        .iter()
        .filter(|row| row.endpoint_kind == endpoint_kind)
        .cloned()
        .collect::<Vec<_>>();
    if !exact_rows.is_empty() {
        return exact_rows;
    }

    for compatible_endpoint_kind in openai_text_endpoint_family(endpoint_kind) {
        let compatible_rows = rows
            .iter()
            .filter(|row| row.endpoint_kind == *compatible_endpoint_kind)
            .cloned()
            .collect::<Vec<_>>();
        if !compatible_rows.is_empty() {
            return compatible_rows;
        }
    }
    Vec::new()
}

fn normalize_model_hint(value: &str) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then_some(trimmed.to_ascii_lowercase())
}

fn collect_model_hint_values(value: &Value, output: &mut Vec<String>) {
    match value {
        Value::String(text) => {
            for item in text.split([',', '\n']).filter_map(normalize_model_hint) {
                output.push(item);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_model_hint_values(item, output);
            }
        }
        _ => {}
    }
}

fn read_model_allow_list_from_payload(payload: &Value) -> Vec<String> {
    let Some(object) = payload.as_object() else {
        return Vec::new();
    };
    let mut values = Vec::new();
    for key in [
        "supportedModels",
        "supported_models",
        "allowedModels",
        "allowed_models",
        "modelCode",
        "model_code",
        "modelId",
        "model_id",
        "model",
        "defaultModel",
        "default_model",
    ] {
        if let Some(value) = object.get(key) {
            collect_model_hint_values(value, &mut values);
        }
    }
    values.sort();
    values.dedup();
    values
}

fn read_model_block_list_from_payload(payload: &Value) -> Vec<String> {
    let Some(object) = payload.as_object() else {
        return Vec::new();
    };
    let mut values = Vec::new();
    for key in [
        "excludedModels",
        "excluded_models",
        "blockedModels",
        "blocked_models",
    ] {
        if let Some(value) = object.get(key) {
            collect_model_hint_values(value, &mut values);
        }
    }
    values.sort();
    values.dedup();
    values
}

pub(super) fn credential_payload_supports_route_row(
    credential_payload: &Value,
    row: &ProjectedPlatformAccessRow,
) -> bool {
    let model_candidates = [Some(row.model_code.as_str()), row.upstream_model.as_deref()]
        .into_iter()
        .flatten()
        .filter_map(normalize_model_hint)
        .collect::<Vec<_>>();
    if model_candidates.is_empty() {
        return true;
    }

    let block_list = read_model_block_list_from_payload(credential_payload);
    if model_candidates
        .iter()
        .any(|candidate| block_list.iter().any(|blocked| blocked == candidate))
    {
        return false;
    }

    let allow_list = read_model_allow_list_from_payload(credential_payload);
    if allow_list.is_empty() {
        return true;
    }

    model_candidates
        .iter()
        .any(|candidate| allow_list.iter().any(|allowed| allowed == candidate))
}
