//! Web Cookie reuse stays bound to its original browser audience and realm.
use crate::routing::config::{ProviderConfigYaml, ProviderCredentialYaml};
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub(super) fn owns(
    provider: &ProviderConfigYaml,
    credential: Option<&ProviderCredentialYaml>,
) -> bool {
    provider.preset.as_deref() == Some("lumalabs")
        || provider.adapter.as_deref().is_some_and(|adapter| {
            crate::routing::candidate::canonicalize_adapter_name(adapter) == "lumalabs_compatible"
        })
        || provider.protocol_profile.as_deref().is_some_and(|profile| {
            crate::protocol::registry::canonicalize_protocol_profile_key(profile) == "lumalabs"
        })
        || url::Url::parse(
            credential
                .and_then(|c| c.base_url.as_deref())
                .unwrap_or(&provider.base_url),
        )
        .is_ok_and(|url| url.host_str() == Some("app.lumalabs.ai"))
}

pub(super) fn binding(
    provider: &ProviderConfigYaml,
    credential: Option<&ProviderCredentialYaml>,
) -> Value {
    if !owns(provider, credential) {
        return Value::Null;
    }
    // Match runtime merge precedence: exact credential keys override provider keys before alias lookup.
    let mut realms: BTreeMap<_, _> = provider
        .extra_body
        .iter()
        .filter(|(key, _)| is_realm(key))
        .collect();
    let mut agents: BTreeMap<_, _> = provider
        .headers
        .iter()
        .filter(|(key, _)| key.eq_ignore_ascii_case("user-agent"))
        .collect();
    if let Some(credential) = credential {
        realms.extend(
            credential
                .extra_body
                .iter()
                .filter(|(key, _)| is_realm(key)),
        );
        agents.extend(
            credential
                .headers
                .iter()
                .filter(|(key, _)| key.eq_ignore_ascii_case("user-agent")),
        );
    }
    json!({"realms":realms,"user_agent":agents,
        "expires_at":credential.and_then(|c|c.expires_at.as_ref()).or(provider.expires_at.as_ref())})
}

fn is_realm(key: &str) -> bool {
    matches!(key, "realmId" | "realm_id" | "boardId" | "board_id")
}
