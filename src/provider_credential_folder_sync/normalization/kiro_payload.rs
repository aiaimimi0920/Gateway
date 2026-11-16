//! Bind imported material to the selected DB account before it can be persisted or sent.
use super::super::canonicalization::{
    canonicalize_credential_material_kind, canonicalize_folder_service_provider_slug,
    canonicalize_folder_surface_slug,
};
use super::kiro_fields::{invalid, key, string, Fields};
use crate::protocol::kiro_oauth::business;
use crate::routing::candidate::{deserialize_provider_payload, ProviderExecutionMode};
use crate::{credential_runtime::SessionAuthConfig, db, error::GatewayError};
use serde_json::{json, Map, Value};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

pub(super) fn project(
    account: &db::GatewayProviderAccountView,
    mut fields: Fields,
    access: &str,
    machine: &str,
    expiry: Option<String>,
    kind: Option<&str>,
) -> Result<Map<String, Value>, GatewayError> {
    if account.adapter != "kiro_compatible"
        || account.protocol_family != "kiro"
        || account.protocol_profile != "kiro"
        || account.execution_mode != ProviderExecutionMode::DirectHttp
        || account
            .endpoint_execution_modes
            .as_ref()
            .is_some_and(|modes| {
                modes
                    .values()
                    .any(|mode| *mode != ProviderExecutionMode::DirectHttp)
            })
        || canonicalize_folder_service_provider_slug(&account.service_provider_key)
            != "kiro-platform"
        || kind.is_some_and(|kind| canonicalize_credential_material_kind(kind) != "bearer_token")
    {
        return Err(invalid());
    }
    for (aliases, expected, canonical) in [
        (
            &["serviceproviderkey"][..],
            canonicalize_folder_service_provider_slug(&account.service_provider_key),
            0,
        ),
        (&["providersurfacekey"][..], "kiro-compatible".into(), 1),
        (
            &["credentialmaterialkind", "materialkind"][..],
            "bearer_token".into(),
            2,
        ),
    ] {
        if let Some(value) = string(fields.root(aliases)?)? {
            let value = match canonical {
                0 => canonicalize_folder_service_provider_slug(&value),
                1 => canonicalize_folder_surface_slug(&value),
                _ => canonicalize_credential_material_kind(&value),
            };
            if value != expected {
                return Err(invalid());
            }
        }
    }
    let api_region = string(fields.read(&["apiregion", "kiroapiregion"])?)?;
    let session = fields.root(&["sessionauth"])?;
    let mut session: SessionAuthConfig = match session {
        None => serde_json::from_value(json!({"transport":"bearer"})).unwrap(),
        Some(value) => serde_json::from_value(value).map_err(|_| invalid())?,
    };
    if session.primary_cookie_name.is_some() || session.secondary_cookie_name.is_some() {
        return Err(invalid());
    }
    if let Some(expiry) = expiry {
        if session
            .expires_at
            .as_ref()
            .is_some_and(|old| old != &expiry)
        {
            return Err(invalid());
        }
        session.expires_at = Some(expiry);
    }
    if let Some(expiry) = &session.expires_at {
        OffsetDateTime::parse(expiry, &Rfc3339).map_err(|_| invalid())?;
    }
    let mut payload = Map::new();
    // Route JSON and DB payload have different alias contracts. Match the selected account's
    // spelling so their real deep merge cannot manufacture duplicate serde fields.
    for (snake, camel, value) in [
        ("api_key", "apiKey", json!(access)),
        ("session_auth", "sessionAuth", json!(session)),
    ] {
        payload.insert(aligned(account, snake, camel)?, value);
    }
    if let Some(expiry) = &session.expires_at {
        payload.insert(aligned(account, "expires_at", "expiresAt")?, json!(expiry));
    }
    for (snake, camel, aliases) in [
        ("base_url", "baseUrl", &["baseurl"][..]),
        ("responses_path", "responsesPath", &["responsespath"][..]),
        ("default_model", "defaultModel", &["defaultmodel"][..]),
        ("account_name", "accountName", &["accountname"][..]),
        ("execution_mode", "executionMode", &["executionmode"][..]),
        (
            "endpoint_execution_modes",
            "endpointExecutionModes",
            &["endpointexecutionmodes"][..],
        ),
        ("call_flow", "callFlow", &["callflow"][..]),
        (
            "request_client_profile",
            "requestClientProfile",
            &["requestclientprofile"][..],
        ),
    ] {
        if let Some(value) = fields.root(aliases)? {
            if snake == "base_url"
                && account
                    .payload
                    .get(snake)
                    .or_else(|| account.payload.get(camel))
                    != Some(&value)
            {
                return Err(invalid());
            }
            payload.insert(aligned(account, snake, camel)?, value);
        }
    }
    for (aliases, output) in [
        (&["headers"][..], "headers"),
        (&["adapter"][..], "adapter"),
        (&["supportedmodels"][..], "supportedModels"),
        (&["credentialmaterialkey"][..], "credentialMaterialKey"),
    ] {
        if let Some(value) = fields.root(aliases)? {
            payload.insert(output.into(), value);
        }
    }
    // No unconsumed authentication, lifetime defaults, alternate origins or nested raw copies.
    if !fields.root.is_empty() || fields.extra.keys().any(|name| business::auth_extra(name)) {
        return Err(invalid());
    }
    // Shadow each inherited spelling with the same normalized non-secret identity. Otherwise
    // a legacy UUID alias would coexist with the fixed hex ID and become order-dependent.
    if let Some(inherited) = account
        .payload
        .get("extra_body")
        .or_else(|| account.payload.get("extraBody"))
        .and_then(Value::as_object)
    {
        for name in inherited
            .keys()
            .filter(|name| matches!(key(name).as_str(), "machineid" | "kiromachineid"))
        {
            fields.extra.insert(name.clone(), json!(machine));
        }
    }
    fields.extra.insert("kiroMachineId".into(), json!(machine));
    payload.insert(
        aligned(account, "extra_body", "extraBody")?,
        Value::Object(fields.extra),
    );
    let merged = db::provider_credentials::merge_provider_account_and_credential_payloads(
        &account.payload,
        &Value::Object(payload.clone()),
    );
    let parsed =
        deserialize_provider_payload(&merged, Some(&account.adapter)).map_err(|_| invalid())?;
    let parsed = crate::routing::call_flow::prepare_owned(parsed).map_err(|_| invalid())?;
    if parsed.api_key != access {
        return Err(invalid());
    }
    business::validate_material_payload(&parsed, machine).map_err(|_| invalid())?;
    if api_region.is_some_and(|region| {
        let host = format!("q.{region}.amazonaws.com");
        url::Url::parse(&parsed.base_url)
            .ok()
            .and_then(|url| url.host_str().map(str::to_owned))
            .as_deref()
            != Some(host.as_str())
    }) {
        return Err(invalid());
    }
    Ok(payload)
}

fn aligned(
    account: &db::GatewayProviderAccountView,
    snake: &str,
    camel: &str,
) -> Result<String, GatewayError> {
    if account.payload.get(snake).is_some() && account.payload.get(camel).is_some() {
        return Err(invalid());
    }
    Ok(if account.payload.get(camel).is_some() {
        camel
    } else {
        snake
    }
    .into())
}
