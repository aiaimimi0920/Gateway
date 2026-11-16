//! The two CLI fingerprint values have their own nested SecretPatch targets.
use super::{descriptors::push_optional_string_descriptor, validation::ensure_absent_option};
use super::{SecretDescriptor, SecretPatchError};
use crate::routing::config::{ProviderConfigYaml, ProviderCredentialYaml, RouteConfigYaml};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap};

/// Keep may retain the token, but changing its explicit business audience needs fresh secret access.
/// Match stable identities, not array positions; deleting an account does not transfer its authority.
pub(crate) fn business_target_changed(active: &RouteConfigYaml, draft: &RouteConfigYaml) -> bool {
    let old: HashMap<_, _> = active
        .providers
        .iter()
        .map(|p| (p.id.as_str(), p))
        .collect();
    draft.providers.iter().any(|provider| {
        let accounts: HashMap<_, _> = old
            .get(provider.id.as_str())
            .into_iter()
            .flat_map(|p| &p.credentials)
            .filter_map(|c| c.id.as_deref().map(|id| (id, c)))
            .collect();
        provider.credentials.iter().any(|credential| {
            let previous = credential.id.as_deref().and_then(|id| accounts.get(id));
            let key = crate::protocol::freebuff::business_target::CONFIG_KEY;
            previous.and_then(|c| c.extra_body.get(key)) != credential.extra_body.get(key)
        })
    })
}

pub(super) fn redact(
    c: &mut ProviderCredentialYaml,
    path: &str,
    secrets: &mut Vec<SecretDescriptor>,
) {
    if let Some(context) = &mut c.freebuff_session {
        for (field, value) in ["fingerprint_id", "fingerprint_hash"]
            .into_iter()
            .zip(context.take_secrets())
        {
            push_optional_string_descriptor(
                secrets,
                format!("{path}/freebuff_session/{field}"),
                value.as_deref(),
            );
        }
    }
}
pub(super) fn ensure_redacted(
    c: &ProviderCredentialYaml,
    path: &str,
) -> Result<(), SecretPatchError> {
    if let Some(context) = &c.freebuff_session {
        for (field, value) in [
            ("fingerprint_id", context.fingerprint_id()),
            ("fingerprint_hash", context.fingerprint_hash()),
        ] {
            ensure_absent_option(value, &format!("{path}/freebuff_session/{field}"))?;
        }
    }
    Ok(())
}
pub(super) fn binding(p: &ProviderConfigYaml, c: Option<&ProviderCredentialYaml>) -> Value {
    let mut paths: BTreeMap<_, _> = p.extra_body.iter().filter(|(k, _)| path_key(k)).collect();
    if let Some(c) = c {
        paths.extend(c.extra_body.iter().filter(|(k, _)| path_key(k)));
    }
    json!({"context":c.and_then(|c|c.freebuff_session.as_ref()), "paths":paths})
}
fn path_key(key: &str) -> bool {
    crate::routing::config::freebuff_session::path_key(key)
}
