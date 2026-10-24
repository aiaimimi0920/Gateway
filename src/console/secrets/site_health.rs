//! Monitoring auth is a route secret. Plan IDs, never array slots, own its identity.
use super::{
    descriptors::push_optional_string_descriptor,
    identity::SecretDisposition,
    pointers::{format_pointer, parse_array_index, ParsedPointer},
    validation::ensure_absent_option,
    SecretDescriptor, SecretPatchError,
};
use crate::routing::config::ProviderConfigYaml;
use std::collections::HashSet;

pub(super) fn redact(
    provider: &mut ProviderConfigYaml,
    base: &str,
    secrets: &mut Vec<SecretDescriptor>,
) {
    let Some(health) = &mut provider.site_health else {
        return;
    };
    push_optional_string_descriptor(
        secrets,
        format!("{base}/site_health/auth/token"),
        health.auth.token.as_deref(),
    );
    health.auth.token = None;
    for (i, p) in health.profiles.iter_mut().enumerate() {
        push_optional_string_descriptor(
            secrets,
            format!("{base}/site_health/profiles/{i}/policy/auth/token"),
            p.policy.auth.token.as_deref(),
        );
        p.policy.auth.token = None;
    }
}
pub(super) fn ensure_redacted(
    provider: &ProviderConfigYaml,
    base: &str,
) -> Result<(), SecretPatchError> {
    let Some(health) = &provider.site_health else {
        return Ok(());
    };
    ensure_absent_option(
        health.auth.token.as_deref(),
        &format!("{base}/site_health/auth/token"),
    )?;
    for (i, p) in health.profiles.iter().enumerate() {
        ensure_absent_option(
            p.policy.auth.token.as_deref(),
            &format!("{base}/site_health/profiles/{i}/policy/auth/token"),
        )?;
    }
    Ok(())
}
pub(super) fn ensure_ids(
    provider: &ProviderConfigYaml,
    base: &str,
) -> Result<(), SecretPatchError> {
    let Some(health) = &provider.site_health else {
        return Ok(());
    };
    let mut ids = HashSet::new();
    for (i, p) in health.profiles.iter().enumerate() {
        if p.id.is_empty()
            || p.id.len() > 64
            || !p
                .id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
            || !ids.insert(&p.id)
        {
            return Err(SecretPatchError::new(
                "secret_document_identity_invalid",
                Some(format!("{base}/site_health/profiles/{i}/id")),
                "monitoring plan IDs must be non-empty and unique",
            ));
        }
    }
    Ok(())
}
pub(super) fn secret_exists(
    provider: &ProviderConfigYaml,
    pointer: &ParsedPointer,
) -> Result<bool, SecretPatchError> {
    let Some(health) = &provider.site_health else {
        return Ok(false);
    };
    let s = &pointer.segments;
    if s.len() == 5 && s[3] == "auth" && s[4] == "token" {
        return Ok(true);
    }
    if s.len() == 8 && s[3] == "profiles" && s[5] == "policy" && s[6] == "auth" && s[7] == "token" {
        return Ok(health
            .profiles
            .get(parse_array_index(&s[4], pointer)?)
            .is_some());
    }
    Ok(false)
}
pub(super) fn map_keep(
    active: &ProviderConfigYaml,
    draft: &ProviderConfigYaml,
    pointer: &ParsedPointer,
    segments: &mut [String],
) -> Result<(), SecretPatchError> {
    if segments[2] != "site_health" {
        return Ok(());
    }
    let error = || {
        SecretPatchError::new(
            "secret_keep_identity_changed",
            Some(pointer.canonical.clone()),
            "monitoring plan identity does not exist in the active revision",
        )
    };
    let active_health = active.site_health.as_ref().ok_or_else(error)?;
    let draft_health = draft.site_health.as_ref().ok_or_else(error)?;
    let (source_policy, target_policy) = if segments.len() == 8 {
        let index = parse_array_index(&segments[4], pointer)?;
        let profile = draft_health.profiles.get(index).ok_or_else(error)?;
        let source = active_health
            .profiles
            .iter()
            .position(|p| p.id == profile.id)
            .ok_or_else(error)?;
        segments[4] = source.to_string();
        (&active_health.profiles[source].policy, &profile.policy)
    } else {
        (&active_health.policy, &draft_health.policy)
    };
    // Keep is allowed without a secret grant. It must never turn ordinary metadata
    // editing into a way to forward an existing secret to a different audience.
    if source_policy.auth.token.is_some()
        && auth_destination(active, source_policy) != auth_destination(draft, target_policy)
    {
        return Err(SecretPatchError::new("secret_keep_destination_changed",Some(pointer.canonical.clone()),
            "monitoring destination or authentication changed; explicitly replace or clear the credential with secret access"));
    }
    Ok(())
}

fn auth_destination(
    provider: &ProviderConfigYaml,
    policy: &crate::site_health::ProbePolicy,
) -> (
    String,
    crate::site_health::HealthAdapter,
    crate::site_health::MonitoringAuthKind,
    Option<String>,
    bool,
) {
    let raw = policy.site_url.as_deref().unwrap_or(&provider.base_url);
    // Empty provider URLs inherit a preset; preserve that identity even before
    // provider compilation/environment expansion has resolved the final origin.
    let origin = if raw.is_empty() {
        format!("preset:{:?}", provider.preset)
    } else {
        url::Url::parse(raw)
            .map(|u| u.origin().ascii_serialization())
            .unwrap_or_else(|_| raw.into())
    };
    let header = (policy.auth.kind == crate::site_health::MonitoringAuthKind::ApiKey).then(|| {
        policy
            .auth
            .header
            .as_deref()
            .unwrap_or("x-api-key")
            .to_ascii_lowercase()
    });
    (
        origin,
        policy.adapter,
        policy.auth.kind,
        header,
        policy.allow_private_network,
    )
}
pub(super) fn draft_path(
    active: &ProviderConfigYaml,
    draft: &ProviderConfigYaml,
    pointer: &ParsedPointer,
    mut segments: Vec<String>,
) -> Result<SecretDisposition, SecretPatchError> {
    let Some(draft) = &draft.site_health else {
        return Ok(SecretDisposition::Deleted);
    };
    if segments.len() == 5 {
        return Ok(SecretDisposition::Retained(format_pointer(&segments)));
    }
    let active = active
        .site_health
        .as_ref()
        .expect("descriptor has active config");
    let index = parse_array_index(&segments[4], pointer)?;
    let profile = &active.profiles[index];
    if let Some(target) = draft.profiles.iter().position(|p| p.id == profile.id) {
        segments[4] = target.to_string();
        return Ok(SecretDisposition::Retained(format_pointer(&segments)));
    }
    // A removed plan is deletion; replacing its old slot with a new identity must
    // explicitly replace the secret, never accidentally inherit the old token.
    let replacement = draft
        .profiles
        .get(index)
        .is_some_and(|p| !active.profiles.iter().any(|a| a.id == p.id));
    Ok(if replacement {
        SecretDisposition::Replacement(format_pointer(&segments))
    } else {
        SecretDisposition::Deleted
    })
}
