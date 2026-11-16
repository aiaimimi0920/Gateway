//! Read-only expansion of the exact old folder shape; storage and row authority never change.
use crate::error::GatewayError;
use crate::protocol::accio_oauth::{import_access, valid_atom, ImportMaterial, GATEWAY};
use serde_json::{Map, Value};

pub(super) fn project(mut payload: Value) -> Result<Value, GatewayError> {
    let Some(current) = payload.as_object() else {
        return Ok(payload);
    };
    if current.get("providerSurfaceKey").and_then(Value::as_str) != Some("accio")
        || current.contains_key("extra_body")
        || current.contains_key("base_url")
    {
        return Ok(payload);
    }
    let Some(base) = current.get("baseUrl").and_then(Value::as_str) else {
        return Ok(payload);
    };
    if url::Url::parse(base).ok() != url::Url::parse(GATEWAY).ok() {
        return Ok(payload);
    }
    let Some(source) = current.get("rawSource").and_then(Value::as_object) else {
        return Ok(payload);
    };
    let Some(device) = source_device(source) else {
        return Ok(payload);
    };
    let Some(body) = current.get("extraBody").and_then(Value::as_object) else {
        return Ok(payload);
    };
    let Some(headers) = current.get("headers").and_then(Value::as_object) else {
        return Ok(payload);
    };
    let Some(body_access) = body
        .get("token")
        .and_then(Value::as_str)
        .filter(|value| valid_atom(value, 16_384))
    else {
        return Ok(payload);
    };
    if body.get("accessToken").and_then(Value::as_str) != Some(body_access)
        || body.get("utdid").and_then(Value::as_str) != Some(device)
        || headers.get("utdid").and_then(Value::as_str) != Some(device)
        || headers.get("x-utdid").and_then(Value::as_str) != Some(device)
        || !current.contains_key("apiKey")
    {
        return Ok(payload);
    }
    // Unknown/manual shapes remain untouched even if their archive contains invalid aliases.
    let Some(access) = import_access(source)? else {
        return Ok(payload);
    };
    if access != body_access {
        return Ok(payload);
    }
    let mut material = Map::new();
    ImportMaterial::parse(source, device, base)?.apply(&mut material);
    // A source archive may describe the previous grant. Never replenish it after rotation/clear.
    let mut same_grant = current.get("apiKey").and_then(Value::as_str) == Some(&access)
        && same_refresh(current, material.get("refreshToken"));
    let original_expiry = current
        .get("expiresAt")
        .and_then(Value::as_str)
        .is_some_and(|at| {
            source
                .get("expiresAt")
                .or_else(|| source.get("expires_at"))
                .and_then(Value::as_str)
                .is_some_and(|original| at == original.trim())
        });
    // A partial clear/changed SG, client, endpoint or deadline also revokes archive replenishment.
    for (key, aliases) in [
        (
            "accioSgcookie",
            &["accioSgcookie", "accio_sgcookie", "sgcookie"][..],
        ),
        (
            "refreshClientId",
            &[
                "refreshClientId",
                "refresh_client_id",
                "clientId",
                "client_id",
            ][..],
        ),
        (
            "refreshEndpoint",
            &["refreshEndpoint", "refresh_endpoint"][..],
        ),
        ("expiresAt", &["expiresAt", "expires_at"][..]),
    ] {
        for (stored_key, value) in aliases
            .iter()
            .filter_map(|alias| current.get(*alias).map(|value| (*alias, value)))
        {
            let original_date = key == "expiresAt" && stored_key == "expiresAt" && original_expiry;
            same_grant &= original_date || (value.is_string() && material.get(key) == Some(value));
        }
        same_grant &= aliases.iter().all(|alias| !body.contains_key(*alias));
    }
    let mut additions = Map::new();
    if same_grant {
        for (key, aliases) in [
            (
                "accioSgcookie",
                &["accioSgcookie", "accio_sgcookie", "sgcookie"][..],
            ),
            (
                "refreshClientId",
                &[
                    "refreshClientId",
                    "refresh_client_id",
                    "clientId",
                    "client_id",
                ][..],
            ),
            (
                "refreshEndpoint",
                &["refreshEndpoint", "refresh_endpoint"][..],
            ),
            ("expiresAt", &["expiresAt", "expires_at"][..]),
        ] {
            // Presence includes null and malformed values: an explicit clear is not legacy absence.
            let absent = aliases
                .iter()
                .all(|alias| !current.contains_key(*alias) && !body.contains_key(*alias));
            let normalize_expiry =
                key == "expiresAt" && original_expiry && !current.contains_key("expires_at");
            if absent || normalize_expiry {
                if let Some(value) = material.get(key) {
                    additions.insert(key.into(), value.clone());
                }
            }
        }
    }
    // Recognition above proves these are copies of the archived grant, not arbitrary user body data.
    let current = payload.as_object_mut().expect("recognized legacy object");
    let body = current
        .get_mut("extraBody")
        .and_then(Value::as_object_mut)
        .expect("recognized legacy body");
    for key in ["token", "accessToken", "utdid"] {
        body.remove(key);
    }
    current.extend(additions);
    Ok(payload)
}

fn same_refresh(current: &Map<String, Value>, expected: Option<&Value>) -> bool {
    let values: Vec<_> = ["refreshToken", "refresh_token"]
        .into_iter()
        .filter_map(|key| current.get(key))
        .collect();
    if values.is_empty() {
        return expected.is_none();
    }
    values
        .into_iter()
        .all(|value| expected == Some(value) && value.is_string())
}

fn source_device(source: &Map<String, Value>) -> Option<&str> {
    let headers = source.get("headers").and_then(Value::as_object);
    let mut selected = None;
    for value in [
        source.get("utdid"),
        source.get("x-utdid"),
        headers.and_then(|map| map.get("utdid")),
        headers.and_then(|map| map.get("x-utdid")),
    ]
    .into_iter()
    .flatten()
    {
        let value = value.as_str()?.trim();
        if !valid_atom(value, 1024) || selected.is_some_and(|previous| previous != value) {
            return None;
        }
        selected = Some(value);
    }
    selected
}
