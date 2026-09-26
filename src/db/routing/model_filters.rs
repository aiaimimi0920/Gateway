use super::*;

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

pub(super) fn credential_payload_supports_model(
    credential_payload: &Value,
    model_alias: Option<&str>,
    upstream_model: Option<&str>,
) -> bool {
    let model_candidates = [model_alias, upstream_model]
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

pub(super) fn provider_allowed_by_route_policy(
    provider_account: &GatewayProviderRouteRow,
    route_policy: Option<&GatewayRoutePolicyConfig>,
) -> bool {
    let allowed_provider_ids = normalize_string_list(
        route_policy.and_then(|policy| policy.allowed_provider_account_ids.clone()),
        false,
    );
    if let Some(allowed_provider_ids) = allowed_provider_ids {
        if !allowed_provider_ids
            .iter()
            .any(|value| value == &provider_account.id)
        {
            return false;
        }
    }

    let allowed_protocol_families = normalize_string_list(
        route_policy.and_then(|policy| policy.allowed_protocol_families.clone()),
        true,
    );
    if let Some(allowed_protocol_families) = allowed_protocol_families {
        if !allowed_protocol_families.iter().any(|value| {
            route_policy_family_matches_surface(
                value,
                &provider_account.adapter,
                &provider_account.protocol_family,
            )
        }) {
            return false;
        }
    }

    true
}

pub(super) fn route_policy_has_model_restrictions(
    route_policy: Option<&GatewayRoutePolicyConfig>,
) -> bool {
    normalize_string_list(
        route_policy.and_then(|policy| policy.allowed_model_ids.clone()),
        true,
    )
    .is_some()
        || normalize_string_list(
            route_policy.and_then(|policy| policy.blocked_model_ids.clone()),
            true,
        )
        .is_some()
}

pub(super) fn route_policy_allows_models<'a>(
    route_policy: Option<&GatewayRoutePolicyConfig>,
    candidate_model_ids: [Option<&'a str>; 2],
) -> bool {
    let blocked_model_ids = normalize_string_list(
        route_policy.and_then(|policy| policy.blocked_model_ids.clone()),
        true,
    );
    let normalized_candidates = candidate_model_ids
        .into_iter()
        .flatten()
        .map(|value| value.trim().to_lowercase())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();

    if let Some(blocked_model_ids) = blocked_model_ids {
        if normalized_candidates
            .iter()
            .any(|value| blocked_model_ids.iter().any(|blocked| blocked == value))
        {
            return false;
        }
    }

    let allowed_model_ids = normalize_string_list(
        route_policy.and_then(|policy| policy.allowed_model_ids.clone()),
        true,
    );
    let Some(allowed_model_ids) = allowed_model_ids else {
        return true;
    };
    if normalized_candidates.is_empty() {
        return false;
    }
    normalized_candidates
        .iter()
        .any(|value| allowed_model_ids.iter().any(|allowed| allowed == value))
}
