//! Static CLI Key folders require a separate account audience; never convert an OAuth pool.
use super::super::canonicalization::{
    canonicalize_credential_material_kind, canonicalize_folder_service_provider_slug,
    canonicalize_folder_surface_slug,
};
use super::kiro_fields::{key as field_key, string, Fields};
use crate::routing::candidate::{deserialize_provider_payload, ProviderExecutionMode};
use crate::{db, error::GatewayError, protocol::kiro_api_key as key};
use serde_json::{json, Map, Value};

pub(in crate::provider_credential_folder_sync) fn claims(payload: &Value) -> bool {
    payload.as_object().is_some_and(|map| {
        claims_fields(map)
            || map.iter().any(|(name, value)| {
                // The legacy normalizer reads this one material container. Do not recurse
                // into arbitrary metadata or archival rawSource copies.
                field_key(name) == "extrabody" && value.as_object().is_some_and(claims_fields)
            })
    })
}

fn claims_fields(map: &Map<String, Value>) -> bool {
    map.iter().any(|(name, value)| {
        let name = field_key(name);
        name == "kiroapikey"
            || value.as_str().is_some_and(|value| match name.as_str() {
                "baseurl" => key::owns_base(value),
                "authmode" => value == key::AUTH_MODE,
                "preset" | "providersurfacekey" => {
                    canonicalize_folder_surface_slug(value) == key::PRESET_ID
                }
                _ => false,
            })
    })
}

pub(in crate::provider_credential_folder_sync) fn normalize(
    account: &db::GatewayProviderAccountView,
    raw: Value,
    kind: Option<&str>,
) -> Result<Value, GatewayError> {
    if account.adapter != "kiro_compatible"
        || account.protocol_profile != "kiro"
        || account.protocol_family != "kiro"
        || canonicalize_folder_service_provider_slug(&account.service_provider_key)
            != "kiro-platform"
        || account.execution_mode != ProviderExecutionMode::DirectHttp
        || account
            .endpoint_execution_modes
            .as_ref()
            .is_some_and(|modes| {
                modes
                    .values()
                    .any(|m| *m != ProviderExecutionMode::DirectHttp)
            })
        || kind.is_some_and(|kind| canonicalize_credential_material_kind(kind) != "api_key")
    {
        return Err(key::invalid());
    }
    // Account JSON is already runtime configuration. Do not activate ignored spellings,
    // inherit a registration/refresh owner, or silently repair a mixed-product account.
    let account_map = account.payload.as_object().ok_or_else(key::invalid)?;
    if account_map.keys().any(|name| {
        let name = field_key(name);
        name != "apikey"
            && (crate::protocol::kiro_oauth::business::auth_extra(&name)
                || name.starts_with("refresh")
                || matches!(
                    name.as_str(),
                    "expiresat"
                        | "tokenexpiresinsecs"
                        | "discovery"
                        | "discoveryjob"
                        | "runtimestateobjectkey"
                        | "machineid"
                        | "kiromachineid"
                ))
    }) {
        return Err(key::invalid());
    }
    let original = deserialize_provider_payload(&account.payload, Some(&account.adapter))
        .map_err(|_| key::invalid())?;
    if !key::owns_payload(&original) {
        return Err(key::invalid());
    }
    key::validate(&original, &account.adapter)?;

    let mut fields = Fields::new(raw).map_err(|_| key::invalid())?;
    let access = string(
        fields
            .root(&["apikey", "kiroapikey"])
            .map_err(|_| key::invalid())?,
    )
    .map_err(|_| key::invalid())?
    .filter(|key| key::valid_material(key))
    .ok_or_else(key::invalid)?;
    if !fields.extra.is_empty() {
        return Err(key::invalid());
    }
    for (aliases, expected, canonical) in [
        (&["serviceproviderkey"][..], "kiro-platform", 0),
        (&["providersurfacekey", "preset"][..], key::PRESET_ID, 1),
        (
            &["credentialmaterialkind", "materialkind"][..],
            "api_key",
            2,
        ),
        (&["authmode"][..], key::AUTH_MODE, 3),
    ] {
        if let Some(value) =
            string(fields.root(aliases).map_err(|_| key::invalid())?).map_err(|_| key::invalid())?
        {
            let value = match canonical {
                0 => canonicalize_folder_service_provider_slug(&value),
                1 => canonicalize_folder_surface_slug(&value),
                2 => canonicalize_credential_material_kind(&value),
                _ => value,
            };
            if value != expected {
                return Err(key::invalid());
            }
        }
    }
    let mut payload = Map::new();
    payload.insert(aligned(account_map, "api_key", "apiKey")?, json!(access));
    // Merge's policy-key owner canonicalizes each layer, so this must stay snake_case.
    payload.insert("auth_mode".into(), json!(key::AUTH_MODE));
    for (snake, camel) in [
        ("base_url", "baseUrl"),
        ("responses_path", "responsesPath"),
        ("default_model", "defaultModel"),
        ("account_name", "accountName"),
        ("execution_mode", "executionMode"),
        ("endpoint_execution_modes", "endpointExecutionModes"),
        ("call_flow", "callFlow"),
        ("request_client_profile", "requestClientProfile"),
    ] {
        if let Some(value) = fields
            .root(&[&field_key(snake)])
            .map_err(|_| key::invalid())?
        {
            let output = if matches!(snake, "call_flow" | "request_client_profile") {
                snake.into()
            } else {
                aligned(account_map, snake, camel)?
            };
            payload.insert(output, value);
        }
    }
    for (aliases, output) in [
        (&["adapter"][..], "adapter"),
        (&["headers"][..], "headers"),
        (&["supportedmodels"][..], "supportedModels"),
    ] {
        if let Some(value) = fields.root(aliases).map_err(|_| key::invalid())? {
            payload.insert(output.into(), value);
        }
    }
    if !fields.root.is_empty() {
        return Err(key::invalid());
    }
    let merged = db::provider_credentials::merge_provider_account_and_credential_payloads(
        &account.payload,
        &Value::Object(payload.clone()),
    );
    let parsed = deserialize_provider_payload(&merged, Some(&account.adapter))
        .map_err(|_| key::invalid())?;
    key::validate(&parsed, &account.adapter)?;
    if parsed.api_key != access
        || parsed
            .default_model
            .as_ref()
            .is_some_and(|m| !crate::provider_model_id::valid_for_payload(m, &parsed))
    {
        return Err(key::invalid());
    }
    if let Some(models) = payload.get("supportedModels") {
        if models.as_array().is_none_or(|models| {
            models.iter().any(|model| {
                model
                    .as_str()
                    .is_none_or(|m| !crate::provider_model_id::valid_for_payload(m, &parsed))
            })
        }) {
            return Err(key::invalid());
        }
    }
    crate::routing::call_flow::prepare_owned(parsed).map_err(|_| key::invalid())?;
    payload.insert(
        "serviceProviderKey".into(),
        json!(account.service_provider_key),
    );
    payload.insert("providerSurfaceKey".into(), json!(key::PRESET_ID));
    payload.insert("credentialMaterialKind".into(), json!("api_key"));
    Ok(Value::Object(payload))
}

fn aligned(account: &Map<String, Value>, snake: &str, camel: &str) -> Result<String, GatewayError> {
    if account.contains_key(snake) && account.contains_key(camel) {
        return Err(key::invalid());
    }
    Ok(if account.contains_key(camel) {
        camel
    } else {
        snake
    }
    .into())
}
