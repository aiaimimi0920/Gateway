//! Keep has no secret grant: protected material cannot silently gain a target or consumer.
#[path = "bedrock_context.rs"]
mod context;
#[path = "luma_web.rs"]
mod luma_web;
#[path = "perplexity_web.rs"]
mod perplexity;
#[path = "qwen_web.rs"]
mod qwen_web;

use super::pointers::{parse_array_index, ParsedPointer};
use super::SecretPatchError;
use crate::routing::config::{ProviderConfigYaml, ProviderCredentialYaml, RouteConfigYaml};
use std::collections::{HashMap, HashSet};

#[derive(Default)]
pub(super) struct KeepBindings {
    pending: Vec<(ParsedPointer, ParsedPointer)>,
    // All products compare the same complete, field-independent destination projection.
    checked: HashSet<(usize, Option<usize>)>,
}

impl KeepBindings {
    pub(super) fn stage(
        &mut self,
        active: &RouteConfigYaml,
        draft: &RouteConfigYaml,
        source: ParsedPointer,
        target: &ParsedPointer,
    ) -> Result<(), SecretPatchError> {
        let old = &active.providers[parse_array_index(&source.segments[1], &source)?];
        let new = &draft.providers[parse_array_index(&target.segments[1], target)?];
        let offset = if target.segments[2] == "credentials" {
            4
        } else {
            2
        };
        if matches!(
            target.segments[offset].as_str(),
            "api_key"
                | "auth_token"
                | "refresh_token"
                | "accio_sgcookie"
                | "kiro_oauth"
                | "quick_oauth"
                | "vertex_oauth"
                | "dim_oauth"
                | "nous_portal"
                | "freebuff_session"
                | "headers"
                | "extra_body"
                | "keepalive"
        ) && (has_protected_product(old) || has_protected_product(new))
        {
            // Do not filter by redacted inheritance: clearing a map override can create a consumer.
            // Candidate paths remain bounded by the normal patch-count/path-byte admission limits.
            self.pending.push((source, target.clone()));
        }
        Ok(())
    }

    pub(super) fn validate_all(
        mut self,
        active: &RouteConfigYaml,
        resolved: &RouteConfigYaml,
    ) -> Result<(), SecretPatchError> {
        for (source, target) in std::mem::take(&mut self.pending) {
            let old = &active.providers[parse_array_index(&source.segments[1], &source)?];
            let new = &resolved.providers[parse_array_index(&target.segments[1], &target)?];
            // Only the complete resolved document determines actual ownership and inheritance.
            if protected_owner(old, &source)? || protected_owner(new, &target)? {
                self.validate(active, resolved, &source, &target)?;
            }
        }
        Ok(())
    }

    fn validate(
        &mut self,
        active: &RouteConfigYaml,
        draft: &RouteConfigYaml,
        source: &ParsedPointer,
        target: &ParsedPointer,
    ) -> Result<(), SecretPatchError> {
        let source_index = parse_array_index(&source.segments[1], source)?;
        let target_index = parse_array_index(&target.segments[1], target)?;
        let active = &active.providers[source_index];
        let draft = &draft.providers[target_index];
        let (field, credential) = if target.segments[2] == "credentials" {
            (
                &target.segments[4..],
                Some(parse_array_index(&target.segments[3], target)?),
            )
        } else {
            (&target.segments[2..], None)
        };
        if let Some(index) = credential {
            let source_credential = parse_array_index(&source.segments[3], source)?;
            return self.require_pair(
                target_index,
                Some(index),
                active,
                draft,
                Some(&active.credentials[source_credential]),
                Some(&draft.credentials[index]),
                target,
            );
        }
        // Bind the provider's own configuration, including an empty pool's future default.
        self.require_pair(target_index, None, active, draft, None, None, target)?;
        let old: HashMap<_, _> = active
            .credentials
            .iter()
            .map(|c| (c.id.as_deref(), c))
            .collect();
        for (index, consumer) in draft.credentials.iter().enumerate() {
            if !inherits(consumer, field) {
                continue;
            }
            let previous = old
                .get(&consumer.id.as_deref())
                .filter(|c| inherits(c, field))
                .ok_or_else(|| changed(target))?;
            // Check all consumers, including generic/disabled accounts, not just protected products.
            self.require_pair(
                target_index,
                Some(index),
                active,
                draft,
                Some(previous),
                Some(consumer),
                target,
            )?;
        }
        Ok(())
    }

    fn require_pair(
        &mut self,
        provider_index: usize,
        credential_index: Option<usize>,
        active: &ProviderConfigYaml,
        draft: &ProviderConfigYaml,
        old: Option<&ProviderCredentialYaml>,
        new: Option<&ProviderCredentialYaml>,
        pointer: &ParsedPointer,
    ) -> Result<(), SecretPatchError> {
        if self.checked.contains(&(provider_index, credential_index)) {
            return Ok(());
        }
        if context::destination(active, old) != context::destination(draft, new)
            || perplexity::binding(active, old) != perplexity::binding(draft, new)
            || qwen_web::binding(active, old) != qwen_web::binding(draft, new)
            || luma_web::binding(active, old) != luma_web::binding(draft, new)
            || crate::protocol::sap_aicore::config_binding(active, old)
                != crate::protocol::sap_aicore::config_binding(draft, new)
        {
            return Err(changed(pointer));
        }
        self.checked.insert((provider_index, credential_index));
        Ok(())
    }
}

fn protected_product(
    provider: &ProviderConfigYaml,
    credential: Option<&ProviderCredentialYaml>,
) -> bool {
    super::bedrock::owns(provider, credential)
        || crate::routing::config::luma_mcp::owns(provider)
        || luma_web::owns(provider, credential)
        || provider.preset.as_deref() == Some("river-api")
        || crate::protocol::luma_inference::config::owns(provider)
        || provider
            .preset
            .as_deref()
            .and_then(crate::protocol::minimax::preset_base)
            .is_some()
        || credential
            .and_then(|c| c.auth_mode.as_deref())
            .or(provider.auth_mode.as_deref())
            == Some(crate::protocol::minimax::AUTH_MODE)
        || provider.preset.as_deref() == Some(crate::protocol::agnes::PRESET)
        || provider.preset.as_deref() == Some(crate::protocol::sensenova::PRESET)
        || provider.preset.as_deref() == Some(crate::protocol::commandcode::PRESET)
        || credential
            .and_then(|c| c.auth_mode.as_deref())
            .or(provider.auth_mode.as_deref())
            == Some(crate::protocol::commandcode::AUTH_MODE)
        || provider.preset.as_deref() == Some(crate::protocol::sap_aicore::PRESET)
        || credential
            .and_then(|c| c.auth_mode.as_deref())
            .or(provider.auth_mode.as_deref())
            == Some(crate::protocol::sap_aicore::AUTH_MODE)
        || credential
            .and_then(|c| c.auth_mode.as_deref())
            .or(provider.auth_mode.as_deref())
            == Some(crate::protocol::sensenova::AUTH_MODE)
        || credential
            .and_then(|c| c.auth_mode.as_deref())
            .or(provider.auth_mode.as_deref())
            == Some(crate::protocol::agnes::AUTH_MODE)
        || provider.preset.as_deref() == Some(crate::protocol::cloudflare_workers_ai::PRESET)
        || credential
            .and_then(|c| c.auth_mode.as_deref())
            .or(provider.auth_mode.as_deref())
            == Some(crate::protocol::cloudflare_workers_ai::AUTH_MODE)
        || crate::protocol::cloudflare_workers_ai::Endpoint::parse(
            credential
                .and_then(|c| c.base_url.as_deref())
                .unwrap_or(&provider.base_url),
        )
        .is_some()
        || provider.preset.as_deref() == Some("freebuff")
        || credential.is_some_and(|c| c.freebuff_session.is_some())
        || crate::routing::config::luma_agents::owns(provider, credential)
        || owns_accio(provider, credential)
        || owns_kiro_key(provider, credential)
        || owns_vertex_express(provider, credential)
        || perplexity::owns_product(provider, credential)
        || qwen_web::owns_product(provider, credential)
        || credential.is_some_and(|c| c.grok_oauth.is_some())
        || credential.is_some_and(|c| c.kiro_oauth.is_some())
        || credential.is_some_and(|c| c.quick_oauth.is_some())
        || provider.preset.as_deref() == Some(crate::protocol::quick_oauth::PRESET_ID)
        || credential
            .and_then(|c| c.auth_mode.as_deref())
            .or(provider.auth_mode.as_deref())
            == Some(crate::protocol::quick_oauth::AUTH_MODE)
        || credential.is_some_and(|c| c.vertex_oauth.is_some())
        || credential.is_some_and(|c| c.dim_oauth.is_some())
        || credential.is_some_and(|c| c.nous_portal.is_some())
        || credential.is_some_and(|c| c.lobsterai.is_some())
        || provider.auth_mode.as_deref() == Some(crate::protocol::lobsterai::AUTH_MODE)
        || provider.auth_mode.as_deref() == Some(crate::protocol::dim_oauth::business::AUTH_MODE)
        || crate::protocol::dim_oauth::business::owns_base(&provider.base_url)
        || credential
            .and_then(|c| c.auth_mode.as_deref())
            .or(provider.auth_mode.as_deref())
            == Some(crate::protocol::vertex_oauth::business::AUTH_MODE)
        || super::qwen_workspace::owns(provider, credential)
        || provider.preset.as_deref() == Some(crate::protocol::grok_oauth::business::PRESET_ID)
        || credential
            .and_then(|c| c.auth_mode.as_deref())
            .or(provider.auth_mode.as_deref())
            == Some(crate::protocol::grok_oauth::business::AUTH_MODE)
        || crate::protocol::grok_oauth::business::owns_base(
            credential
                .and_then(|c| c.base_url.as_deref())
                .unwrap_or(&provider.base_url),
        )
}

fn owns_kiro_key(
    provider: &ProviderConfigYaml,
    credential: Option<&ProviderCredentialYaml>,
) -> bool {
    use crate::protocol::kiro_api_key as key;
    provider.preset.as_deref() == Some(key::PRESET_ID)
        || credential
            .and_then(|c| c.auth_mode.as_deref())
            .or(provider.auth_mode.as_deref())
            == Some(key::AUTH_MODE)
        || key::owns_base(
            credential
                .and_then(|c| c.base_url.as_deref())
                .unwrap_or(&provider.base_url),
        )
}

fn owns_vertex_express(
    provider: &ProviderConfigYaml,
    credential: Option<&ProviderCredentialYaml>,
) -> bool {
    use crate::protocol::vertex_express as key;
    provider.preset.as_deref() == Some(key::PRESET_ID)
        || credential
            .and_then(|c| c.auth_mode.as_deref())
            .or(provider.auth_mode.as_deref())
            == Some(key::AUTH_MODE)
        || key::owns_base(
            credential
                .and_then(|c| c.base_url.as_deref())
                .unwrap_or(&provider.base_url),
        )
}

fn has_protected_product(provider: &ProviderConfigYaml) -> bool {
    protected_product(provider, None)
        || provider
            .credentials
            .iter()
            .any(|c| protected_product(provider, Some(c)))
}

fn owns_accio(provider: &ProviderConfigYaml, credential: Option<&ProviderCredentialYaml>) -> bool {
    provider.preset.as_deref() == Some("accio")
        || provider.adapter.as_deref().is_some_and(|adapter| {
            crate::routing::candidate::canonicalize_adapter_name(adapter) == "accio_compatible"
        })
        || credential.is_some_and(|c| c.accio_sgcookie.is_some())
        || [
            Some(
                credential
                    .and_then(|c| c.base_url.as_deref())
                    .unwrap_or(&provider.base_url),
            ),
            credential.and_then(|c| c.refresh_endpoint.as_deref()),
        ]
        .into_iter()
        .flatten()
        .any(|value| {
            url::Url::parse(value).is_ok_and(|url| url.host_str() == Some("phoenix-gw.alibaba.com"))
        })
}

fn protected_owner(
    provider: &ProviderConfigYaml,
    pointer: &ParsedPointer,
) -> Result<bool, SecretPatchError> {
    if pointer.segments[2] == "credentials" {
        let credential = &provider.credentials[parse_array_index(&pointer.segments[3], pointer)?];
        return Ok(protected_product(provider, Some(credential)));
    }
    Ok(protected_product(provider, None)
        || provider.credentials.iter().any(|credential| {
            inherits(credential, &pointer.segments[2..])
                && protected_product(provider, Some(credential))
        }))
}

fn inherits(credential: &ProviderCredentialYaml, field: &[String]) -> bool {
    match field.first().map(String::as_str) {
        Some("api_key") => credential.api_key.is_none(),
        Some("auth_token") => credential.auth_token.is_none(),
        Some("keepalive") => credential.keepalive.is_none(),
        Some("headers") => field
            .get(1)
            .is_some_and(|key| !credential.headers.contains_key(key)),
        Some("extra_body") => field
            .get(1)
            .is_some_and(|key| !credential.extra_body.contains_key(key)),
        _ => false,
    }
}

fn changed(pointer: &ParsedPointer) -> SecretPatchError {
    SecretPatchError::new(
        "secret_keep_destination_changed",
        Some(pointer.canonical.clone()),
        "Protected authentication, destination or consumer changed; explicitly replace or clear the material with secret access",
    )
}
