//! Document identity, cross-reference and alias diagnostics retain their stable order.

mod urls;

use super::{encode_pointer_segment, RouteConfigDiagnostics};
use crate::routing::config::{
    effective_credential_id, normalized_alias_conflicts, provider_default_account_id,
    RouteConfigYaml,
};
use std::collections::{HashMap, HashSet};
use urls::{
    provider_uses_websocket_transport, validate_http_url, validate_literal_http_url,
    validate_provider_url,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AccountIdentityKind {
    ProviderDefault,
    Credential,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AccountIdentityOrigin {
    kind: AccountIdentityKind,
    provider_index: usize,
    credential_index: Option<usize>,
}

fn register_account_identity(
    account_ids: &mut HashMap<String, AccountIdentityOrigin>,
    account_id: String,
    origin: AccountIdentityOrigin,
    current_path: &str,
    diagnostics: &mut RouteConfigDiagnostics,
) {
    match account_ids.entry(account_id.clone()) {
        std::collections::hash_map::Entry::Vacant(entry) => {
            entry.insert(origin);
        }
        std::collections::hash_map::Entry::Occupied(entry) if entry.get().kind != origin.kind => {
            let first = entry.get();
            let first_path = match first.kind {
                AccountIdentityKind::ProviderDefault => {
                    format!(
                        "/providers/{}/id (provider default account)",
                        first.provider_index
                    )
                }
                AccountIdentityKind::Credential => format!(
                    "/providers/{}/credentials/{}/id",
                    first.provider_index,
                    first.credential_index.unwrap_or_default()
                ),
            };
            diagnostics.push_error(
                "account_identity_duplicate",
                format!("{current_path}/id"),
                format!("account identity '{account_id}' conflicts with {first_path}"),
            );
        }
        std::collections::hash_map::Entry::Occupied(_) => {}
    }
}

pub(super) fn collect_document_diagnostics(document: &RouteConfigYaml) -> RouteConfigDiagnostics {
    let mut diagnostics = RouteConfigDiagnostics::default();
    let mut provider_ids = HashMap::<&str, usize>::new();
    let mut credential_ids = HashMap::<String, (usize, usize)>::new();
    let mut account_ids = HashMap::<String, AccountIdentityOrigin>::new();

    for (provider_index, provider) in document.providers.iter().enumerate() {
        let provider_path = format!("/providers/{provider_index}");
        let normalized_provider_id = provider.id.trim();
        if normalized_provider_id.is_empty() {
            diagnostics.push_error(
                "provider_id_empty",
                format!("{provider_path}/id"),
                "provider ID must not be empty",
            );
        } else if normalized_provider_id != provider.id {
            diagnostics.push_error(
                "provider_id_whitespace",
                format!("{provider_path}/id"),
                "provider ID must not have leading or trailing whitespace",
            );
        }
        if let Some(first_index) = provider_ids.insert(provider.id.as_str(), provider_index) {
            diagnostics.push_error(
                "provider_id_duplicate",
                format!("{provider_path}/id"),
                format!(
                    "provider ID '{}' duplicates /providers/{first_index}/id",
                    provider.id
                ),
            );
        }
        let websocket_transport = provider_uses_websocket_transport(provider);
        validate_provider_url(
            &provider.base_url,
            "provider_base_url_invalid",
            format!("{provider_path}/base_url"),
            websocket_transport,
            &mut diagnostics,
        );
        if let Some(keepalive) = &provider.keepalive {
            validate_http_url(
                &keepalive.service_url,
                "provider_keepalive_url_invalid",
                format!("{provider_path}/keepalive/serviceUrl"),
                &mut diagnostics,
            );
        }

        if provider.credentials.is_empty() {
            let default_account_id = provider_default_account_id(normalized_provider_id);
            register_account_identity(
                &mut account_ids,
                default_account_id,
                AccountIdentityOrigin {
                    kind: AccountIdentityKind::ProviderDefault,
                    provider_index,
                    credential_index: None,
                },
                &provider_path,
                &mut diagnostics,
            );
        }

        for (credential_index, credential) in provider.credentials.iter().enumerate() {
            let credential_path = format!("{provider_path}/credentials/{credential_index}");
            if let Some(raw_id) = credential.id.as_deref() {
                let normalized_id = raw_id.trim();
                if normalized_id.is_empty() {
                    diagnostics.push_error(
                        "credential_id_empty",
                        format!("{credential_path}/id"),
                        "credential ID must not be empty",
                    );
                } else if normalized_id != raw_id {
                    diagnostics.push_error(
                        "credential_id_whitespace",
                        format!("{credential_path}/id"),
                        "credential ID must not have leading or trailing whitespace",
                    );
                }
            }
            let credential_id =
                effective_credential_id(normalized_provider_id, credential_index, credential)
                    .trim()
                    .to_string();
            let duplicate_origin =
                credential_ids.insert(credential_id.clone(), (provider_index, credential_index));
            let credential_is_duplicate = duplicate_origin.is_some();
            if let Some((first_provider, first_credential)) = duplicate_origin {
                diagnostics.push_error(
                    "credential_id_duplicate",
                    format!("{credential_path}/id"),
                    format!(
                        "credential ID '{credential_id}' duplicates /providers/{first_provider}/credentials/{first_credential}/id"
                    ),
                );
            }
            if !credential_is_duplicate {
                register_account_identity(
                    &mut account_ids,
                    credential_id,
                    AccountIdentityOrigin {
                        kind: AccountIdentityKind::Credential,
                        provider_index,
                        credential_index: Some(credential_index),
                    },
                    &credential_path,
                    &mut diagnostics,
                );
            }
            if let Some(base_url) = &credential.base_url {
                validate_provider_url(
                    base_url,
                    "credential_base_url_invalid",
                    format!("{credential_path}/base_url"),
                    websocket_transport,
                    &mut diagnostics,
                );
            }
            if let Some(keepalive) = &credential.keepalive {
                validate_http_url(
                    &keepalive.service_url,
                    "credential_keepalive_url_invalid",
                    format!("{credential_path}/keepalive/serviceUrl"),
                    &mut diagnostics,
                );
            }
            if let Some(refresh_endpoint) = &credential.refresh_endpoint {
                validate_literal_http_url(
                    refresh_endpoint,
                    "credential_refresh_endpoint_invalid",
                    format!("{credential_path}/refresh_endpoint"),
                    &mut diagnostics,
                );
            }
            if let Some(expires_in_secs) = credential.token_expires_in_secs {
                if !token_expiry_supported(expires_in_secs) {
                    diagnostics.push_error(
                        "credential_token_expiry_invalid",
                        format!("{credential_path}/token_expires_in_secs"),
                        "token expiry cannot be represented safely by the runtime clock",
                    );
                }
            }
        }
    }

    let known_providers: HashSet<&str> = document
        .providers
        .iter()
        .map(|provider| provider.id.as_str())
        .collect();
    for (route_index, route) in document.model_routes.iter().enumerate() {
        let route_path = format!("/model_routes/{route_index}");
        if route.pattern.trim().is_empty() {
            diagnostics.push_error(
                "route_pattern_empty",
                format!("{route_path}/pattern"),
                "route pattern must not be empty",
            );
        }
        if route.provider_ids.is_empty() {
            diagnostics.push_error(
                "route_provider_ids_empty",
                format!("{route_path}/provider_ids"),
                "route must reference at least one provider",
            );
        }
        for (provider_index, provider_id) in route.provider_ids.iter().enumerate() {
            if !known_providers.contains(provider_id.as_str()) {
                diagnostics.push_error(
                    "route_provider_unknown",
                    format!("{route_path}/provider_ids/{provider_index}"),
                    format!("route references unknown provider '{provider_id}'"),
                );
            }
        }
    }

    let mut account_group_ids = HashMap::<&str, usize>::new();
    for (group_index, group) in document.account_groups.iter().enumerate() {
        let group_path = format!("/account_groups/{group_index}");
        let group_id = group.id.trim();
        if group_id.is_empty() {
            diagnostics.push_error(
                "account_group_id_empty",
                format!("{group_path}/id"),
                "account group ID must not be empty",
            );
        } else if let Some(first_index) = account_group_ids.insert(group_id, group_index) {
            diagnostics.push_error(
                "account_group_id_duplicate",
                format!("{group_path}/id"),
                format!(
                    "account group ID '{group_id}' duplicates /account_groups/{first_index}/id"
                ),
            );
        }

        if group.name.trim().is_empty() {
            diagnostics.push_error(
                "account_group_name_empty",
                format!("{group_path}/name"),
                "account group name must not be empty",
            );
        }

        if let Some(multiplier) = group.billing_multiplier {
            if !multiplier.is_finite() || multiplier < 0.0 {
                diagnostics.push_error(
                    "account_group_billing_multiplier_invalid",
                    format!("{group_path}/billing_multiplier"),
                    "account group billing multiplier must be a finite number greater than or equal to 0",
                );
            }
        }

        let mut member_ids = HashSet::<&str>::new();
        for (member_index, member_id) in group.provider_credential_ids.iter().enumerate() {
            let member_path = format!("{group_path}/provider_credential_ids/{member_index}");
            let trimmed_member_id = member_id.trim();
            if trimmed_member_id.is_empty() {
                diagnostics.push_error(
                    "account_group_member_empty",
                    member_path,
                    "account group member ID must not be empty",
                );
                continue;
            }
            if !member_ids.insert(trimmed_member_id) {
                diagnostics.push_error(
                    "account_group_member_duplicate",
                    member_path.clone(),
                    format!(
                        "account group member '{trimmed_member_id}' is duplicated within the same group"
                    ),
                );
            }
            if !account_ids.contains_key(trimmed_member_id) {
                diagnostics.push_error(
                    "account_group_member_unknown",
                    member_path,
                    format!("account group references unknown account '{trimmed_member_id}'"),
                );
            }
        }
    }

    collect_alias_diagnostics(&document.aliases, &mut diagnostics);
    for (_normalized, _first_key, second_key, targets) in
        normalized_alias_conflicts(&document.aliases)
    {
        diagnostics.push_error(
            "alias_normalized_collision",
            format!("/aliases/{}", encode_pointer_segment(&second_key)),
            format!("normalized alias key conflicts with another alias ({targets})"),
        );
    }
    diagnostics.sort();
    diagnostics
}

fn token_expiry_supported(seconds: u64) -> bool {
    let duration = std::time::Duration::from_secs(seconds);
    std::time::Instant::now().checked_add(duration).is_some()
        && std::time::SystemTime::now().checked_add(duration).is_some()
}

fn collect_alias_diagnostics(
    aliases: &HashMap<String, String>,
    diagnostics: &mut RouteConfigDiagnostics,
) {
    let mut keys: Vec<&str> = aliases.keys().map(String::as_str).collect();
    keys.sort_unstable();
    let mut reported = HashSet::<String>::new();
    let mut visited = HashSet::<String>::new();

    for start in keys {
        if visited.contains(start) {
            continue;
        }
        let mut positions = HashMap::<String, usize>::new();
        let mut chain = Vec::<String>::new();
        let mut current = start.to_string();
        while aliases.contains_key(&current) && !visited.contains(&current) {
            if let Some(position) = positions.get(&current).copied() {
                let mut cycle = chain[position..].to_vec();
                cycle.sort();
                let signature = cycle.join("\0");
                if reported.insert(signature) {
                    let alias = cycle.first().cloned().unwrap_or(current);
                    diagnostics.push_error(
                        "alias_cycle",
                        format!("/aliases/{}", encode_pointer_segment(&alias)),
                        format!("alias cycle includes {}", cycle.join(" -> ")),
                    );
                }
                break;
            }
            positions.insert(current.clone(), chain.len());
            chain.push(current.clone());
            let Some(next) = aliases.get(&current) else {
                break;
            };
            current = next.clone();
        }
        visited.extend(chain);
    }
}
