//! Restart repair requires an attributable hosted cache entry, not merely matching token strings.
use crate::console::refresh_grants::GrantRecord;
use crate::redis::credential_cache::CredentialEntry;
use crate::routing::candidate::{canonicalize_adapter_name, ProviderAccountPayload};

pub(super) fn matches(raw: &str, record: &GrantRecord, target: &ProviderAccountPayload) -> bool {
    let Ok(entry) = serde_json::from_str::<CredentialEntry>(raw) else {
        return false;
    };
    let Some(base) = entry.api_base_url.as_deref() else {
        return false;
    };
    if entry.id != record.credential_id() {
        return false;
    }
    if record.original().nous_portal.is_some() {
        let Some(context) = record
            .result()
            .and_then(|r| r.material.nous_portal.as_ref())
        else {
            return false;
        };
        if crate::protocol::nous_portal::business::validate_context(target, context).is_err() {
            return false;
        }
    } else if !same_url(base, &target.base_url) {
        return false;
    }
    let preset = entry
        .preset_name()
        .and_then(crate::preset::get_builtin_preset);
    // Build and API-Key products share the OpenAI adapter, not authorization ownership.
    if record.original().context.is_some()
        && preset
            .as_ref()
            .is_none_or(|preset| preset.id != crate::protocol::grok_oauth::business::PRESET_ID)
    {
        return false;
    }
    let adapter = preset.map(|preset| preset.adapter).unwrap_or_else(|| {
        crate::routing::credential_routing::adapter_for_provider(&entry.provider)
    });
    if canonicalize_adapter_name(&adapter) != canonicalize_adapter_name(&target.adapter) {
        return false;
    }
    let Some(payload) = entry
        .account_payload
        .as_ref()
        .and_then(|value| value.as_object())
    else {
        return false;
    };
    let value = |snake: &str, camel: &str| payload.get(snake).or_else(|| payload.get(camel));
    let material = record.original();
    if material.nous_portal.as_ref().is_some_and(|context| {
        value("base_url", "baseUrl").is_some_and(|value| {
            value.as_str().is_none_or(|value| {
                value.trim_end_matches('/') != context.inference_target().base_url()
            })
        })
    }) {
        return false;
    }
    if !matches_nous(
        entry.account_payload.as_ref(),
        material.nous_portal.as_ref(),
    ) || material
        .nous_portal
        .as_ref()
        .is_some_and(|context| base.trim_end_matches('/') != context.inference_target().base_url())
    {
        return false;
    }
    if material.dim_oauth.as_ref().is_some_and(|context| {
        crate::protocol::dim_oauth::business::validate_context(target, context).is_err()
    }) || !matches_dim(entry.account_payload.as_ref(), material.dim_oauth.as_ref())
    {
        return false;
    }
    if material.vertex_oauth.as_ref().is_some_and(|context| {
        crate::protocol::vertex_oauth::business::validate_context(target, context).is_err()
    }) {
        return false;
    }
    if !matches_vertex(
        entry.account_payload.as_ref(),
        material.vertex_oauth.as_ref(),
    ) {
        return false;
    }
    if !matches_kiro(entry.account_payload.as_ref(), material.kiro_oauth.as_ref())
        || !matches_quick(entry.account_payload.as_ref(), material.quick_oauth.as_ref())
    {
        return false;
    }
    if !matches_sgcookie(
        entry.account_payload.as_ref(),
        material.accio_sgcookie.as_deref(),
    ) {
        return false;
    }
    // Preserve Option<String>'s null/missing contract, but never erase an invalid explicit type.
    for value in [
        value("refresh_endpoint", "refreshEndpoint"),
        value("refresh_client_id", "refreshClientId"),
        value("expires_at", "expiresAt"),
    ] {
        if value.is_some_and(|value| !value.is_null() && !value.is_string()) {
            return false;
        }
    }
    let endpoint = value("refresh_endpoint", "refreshEndpoint").and_then(|value| value.as_str());
    let client = value("refresh_client_id", "refreshClientId").and_then(|value| value.as_str());
    if material.context.is_none()
        && material.kiro_oauth.is_none()
        && material.quick_oauth.is_none()
        && material.vertex_oauth.is_none()
        && material.dim_oauth.is_none()
        && material.nous_portal.is_none()
    {
        if !endpoint.is_some_and(|endpoint| same_url(endpoint, &material.endpoint))
            || client.unwrap_or_default() != material.client
        {
            return false;
        }
    } else if endpoint.is_some_and(|endpoint| !same_url(endpoint, &material.endpoint))
        || client.is_some_and(|client| client != material.client)
    {
        return false;
    }
    if value("refresh_token", "refreshToken").and_then(|value| value.as_str())
        != Some(material.refresh.as_str())
    {
        return false;
    }
    let payload_expiry = value("expires_at", "expiresAt").and_then(|value| value.as_str());
    if !material.matches_expiry(entry.expires_at.as_deref().or(payload_expiry))
        || payload_expiry.is_some_and(|expiry| !material.matches_expiry(Some(expiry)))
    {
        return false;
    }
    if let Some(flow) = value("call_flow", "callFlow") {
        let Ok(flow) = serde_json::from_value::<Option<crate::protocol::call_flow::ProviderCallFlow>>(
            flow.clone(),
        ) else {
            return false;
        };
        if flow != target.call_flow {
            return false;
        }
    }
    true
}

pub(super) fn matches_kiro(
    payload: Option<&serde_json::Value>,
    expected: Option<&crate::protocol::kiro_oauth::AuthorizationContext>,
) -> bool {
    let saved = payload
        .and_then(|payload| payload.get("kiro_oauth"))
        .unwrap_or(&serde_json::Value::Null);
    serde_json::to_value(expected).is_ok_and(|expected| *saved == expected)
        && match expected.and_then(|context| context.runtime_target()) {
            Some(target) => crate::protocol::kiro_oauth::runtime::matches_cache(payload, target),
            None => !["auth_mode", "authMode"].iter().any(|name| {
                payload.and_then(|p| p.get(*name)).and_then(|v| v.as_str())
                    == Some(crate::protocol::kiro_oauth::runtime::AUTH_MODE)
            }),
        }
}

pub(super) fn matches_vertex(
    payload: Option<&serde_json::Value>,
    expected: Option<&crate::protocol::vertex_oauth::AuthorizationContext>,
) -> bool {
    let modes: Vec<_> = ["auth_mode", "authMode"]
        .iter()
        .filter_map(|key| payload.and_then(|value| value.get(key)))
        .collect();
    let typed_mode =
        serde_json::Value::String(crate::protocol::vertex_oauth::business::AUTH_MODE.into());
    if expected.is_some() && (modes.is_empty() || modes.iter().any(|mode| **mode != typed_mode))
        || expected.is_none() && modes.iter().any(|mode| **mode == typed_mode)
    {
        return false;
    }
    let saved = payload
        .and_then(|payload| payload.get("vertex_oauth"))
        .unwrap_or(&serde_json::Value::Null);
    serde_json::to_value(expected).is_ok_and(|expected| *saved == expected)
}

fn same_url(left: &str, right: &str) -> bool {
    url::Url::parse(left)
        .ok()
        .zip(url::Url::parse(right).ok())
        .is_some_and(|(left, right)| left == right)
}

pub(super) fn matches_nous(
    payload: Option<&serde_json::Value>,
    expected: Option<&crate::protocol::nous_portal::AuthorizationContext>,
) -> bool {
    let saved = payload
        .and_then(|p| p.get("nous_portal"))
        .unwrap_or(&serde_json::Value::Null);
    serde_json::to_value(expected).is_ok_and(|expected| *saved == expected)
}

pub(super) fn matches_dim(
    payload: Option<&serde_json::Value>,
    expected: Option<&crate::protocol::dim_oauth::AuthorizationContext>,
) -> bool {
    let modes: Vec<_> = ["auth_mode", "authMode"]
        .iter()
        .filter_map(|key| payload.and_then(|value| value.get(key)))
        .collect();
    let typed = serde_json::Value::String(crate::protocol::dim_oauth::business::AUTH_MODE.into());
    if expected.is_some() && (modes.is_empty() || modes.iter().any(|mode| **mode != typed))
        || expected.is_none() && modes.iter().any(|mode| **mode == typed)
    {
        return false;
    }
    let saved = payload
        .and_then(|p| p.get("dim_oauth"))
        .unwrap_or(&serde_json::Value::Null);
    serde_json::to_value(expected).is_ok_and(|expected| *saved == expected)
}

/// Optional null/missing is legacy absence; wrong types and contradictory aliases are not absence.
pub(super) fn matches_sgcookie(
    payload: Option<&serde_json::Value>,
    expected: Option<&str>,
) -> bool {
    let mut present = false;
    for name in ["accio_sgcookie", "accioSgcookie"] {
        if let Some(value) = payload.and_then(|payload| payload.get(name)) {
            present = true;
            if !(value.is_null() && expected.is_none()
                || value.as_str().is_some_and(|value| Some(value) == expected))
            {
                return false;
            }
        }
    }
    present || expected.is_none()
}

pub(super) fn matches_quick(
    payload: Option<&serde_json::Value>,
    expected: Option<&crate::protocol::quick_oauth::AuthorizationContext>,
) -> bool {
    let modes: Vec<_> = ["auth_mode", "authMode"]
        .iter()
        .filter_map(|key| payload.and_then(|value| value.get(key)))
        .collect();
    let typed = serde_json::Value::String(crate::protocol::quick_oauth::AUTH_MODE.into());
    if expected.is_some() && (modes.is_empty() || modes.iter().any(|mode| **mode != typed))
        || expected.is_none() && modes.iter().any(|mode| **mode == typed)
    {
        return false;
    }
    let saved = payload
        .and_then(|p| p.get("quick_oauth"))
        .unwrap_or(&serde_json::Value::Null);
    serde_json::to_value(expected).is_ok_and(|expected| *saved == expected)
}
