//! Explicit reauthorization may retire legacy copies, never infer a registration or rotate the old device.
use crate::{
    protocol::{
        kiro,
        kiro_oauth::{business, OAuthError},
    },
    routing::candidate::ProviderAccountPayload,
};
use serde_json::{json, Value};
use std::collections::HashMap;

fn key(value: &str) -> String {
    value
        .chars()
        .filter(|c| !matches!(c, '_' | '-'))
        .map(|c| c.to_ascii_lowercase())
        .collect()
}
pub(super) fn machine_key(value: &str) -> bool {
    matches!(key(value).as_str(), "machineid" | "kiromachineid")
}
pub(super) fn retired_extra(value: &str) -> bool {
    business::auth_extra(value)
        || matches!(
            key(value).as_str(),
            "directoryregion"
                | "starturl"
                | "kirostarturl"
                | "clientsecretexpiresat"
                | "kiroclientsecretexpiresat"
                | "scopes"
                | "kiroscopes"
                | "refreshendpoint"
                | "refreshclientid"
        )
}

pub(super) fn machine(payload: &ProviderAccountPayload) -> Result<String, OAuthError> {
    let mut explicit = None;
    let mut refresh = None;
    for (name, value) in payload.extra_body.iter().flatten() {
        if machine_key(name) {
            let machine = value
                .as_str()
                .and_then(kiro::normalize_machine_id)
                .ok_or(OAuthError::InvalidInput)?;
            if explicit.as_ref().is_some_and(|old| old != &machine) {
                return Err(OAuthError::InvalidInput);
            }
            explicit = Some(machine);
        } else if matches!(key(name).as_str(), "kirorefreshtoken" | "refreshtoken") {
            let token = value
                .as_str()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .ok_or(OAuthError::InvalidInput)?;
            if refresh.as_deref().is_some_and(|old| old != token) {
                return Err(OAuthError::InvalidInput);
            }
            refresh = Some(token.to_owned());
        }
    }
    // The old header owner did not read the root refresh_token. Keep its exact zero fallback.
    Ok(
        kiro::generate_machine_id(explicit.as_deref(), refresh.as_deref())
            .unwrap_or_else(|| "0".repeat(64)),
    )
}

pub(super) fn validate(payload: &ProviderAccountPayload, expected: &str) -> Result<(), OAuthError> {
    if machine(payload)? != expected {
        return Err(OAuthError::InvalidInput);
    }
    let base = url::Url::parse(&payload.base_url).map_err(|_| OAuthError::InvalidInput)?;
    for (name, value) in payload.extra_body.iter().flatten() {
        match key(name).as_str() {
            "kiroapiregion" | "apiregion" => {
                let region = value
                    .as_str()
                    .map(str::trim)
                    .ok_or(OAuthError::InvalidInput)?;
                if base.host_str() != Some(format!("q.{region}.amazonaws.com").as_str()) {
                    return Err(OAuthError::InvalidInput);
                }
            }
            // Foreign product contexts are not an alternative import method for Kiro OAuth.
            "grokoauth" | "acciosgcookie" | "kirooauth" => return Err(OAuthError::InvalidInput),
            _ => {}
        }
    }
    let mut prospective = payload.clone();
    let extra = prospective.extra_body.get_or_insert_with(Default::default);
    extra.retain(|name, _| !retired_extra(name));
    normalize_device(&HashMap::new(), extra, expected);
    business::validate_material_payload(&prospective, expected)
}

/// Shadow every inherited spelling with the same canonical device before the typed owner merges it.
pub(super) fn normalize_device(
    inherited: &HashMap<String, Value>,
    extra: &mut HashMap<String, Value>,
    machine: &str,
) {
    for name in inherited.keys().filter(|name| machine_key(name)) {
        extra.insert(name.clone(), json!(machine));
    }
    for (_, value) in extra.iter_mut().filter(|(name, _)| machine_key(name)) {
        *value = json!(machine);
    }
    extra.insert("kiroMachineId".into(), json!(machine));
}
