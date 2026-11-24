//! Internal destination projection for protected Keep bindings; never returned or logged.
//! Model/label/test edits are deliberately excluded; every product compares this same projection.
use crate::routing::config::{ProviderConfigYaml, ProviderCredentialYaml};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn destination(
    provider: &ProviderConfigYaml,
    credential: Option<&ProviderCredentialYaml>,
) -> Value {
    let base = credential
        .and_then(|c| c.base_url.as_deref())
        .unwrap_or(&provider.base_url);
    let session = credential
        .and_then(|c| c.session_auth.as_ref())
        .or(provider.session_auth.as_ref());
    let keepalive = credential
        .and_then(|c| c.keepalive.as_ref())
        .or(provider.keepalive.as_ref());
    let discovery = credential.and_then(|c| c.discovery.as_ref()).map(|d| {
        json!({
            "source": d.source_url, "base": d.api_base, "protocol": d.protocol,
            "protocols": d.protocols.iter().map(|p| (&p.protocol, &p.api_base)).collect::<Vec<_>>()
        })
    });
    json!({
        "base": base.trim_end_matches('/'),
        "mode": credential.and_then(|c| c.auth_mode.as_deref()).or(provider.auth_mode.as_deref()).map(str::trim),
        "preset": provider.preset, "adapter": provider.adapter,
        "family": provider.protocol_family, "profile": provider.protocol_profile,
        "flow": provider.call_flow,
        "fingerprint": credential.and_then(|c| c.request_client_profile).or(provider.request_client_profile),
        "basispoints": credential.and_then(|c| c.codex_basispoints_enabled).unwrap_or(provider.codex_basispoints_enabled),
        "session": session,
        "runtime_state": credential.and_then(|c| c.runtime_state_object_key.as_ref()).or(provider.runtime_state_object_key.as_ref()),
        "execution": credential.and_then(|c| c.execution_mode).or(provider.execution_mode),
        "endpoint_modes": credential.and_then(|c| c.endpoint_execution_modes.as_ref()).or(provider.endpoint_execution_modes.as_ref()),
        "keepalive": keepalive.map(|k| (&k.service_url, &k.ensure_path)),
        "refresh": credential.map(|c| (&c.refresh_endpoint, &c.refresh_client_id)),
        "grok_oauth": credential.and_then(|c| c.grok_oauth.as_ref()),
        "luma_oauth": credential.and_then(|c| c.luma_oauth.as_ref()),
        "kiro_oauth": credential.and_then(|c| c.kiro_oauth.as_ref()),
        "quick_oauth": credential.and_then(|c| c.quick_oauth.as_ref()),
        "kiro_runtime_profile": kiro_runtime_profile(provider, credential),
        "vertex_oauth": credential.and_then(|c| c.vertex_oauth.as_ref()),
        "dim_oauth": credential.and_then(|c| c.dim_oauth.as_ref()),
        "nous_portal": credential.and_then(|c| c.nous_portal.as_ref()),
        "lobsterai": credential.and_then(|c| c.lobsterai.as_ref()),
        "freebuff": super::super::freebuff_session::binding(provider, credential),
        "discovery": discovery, "regions": regions(provider, credential),
        "paths": [&provider.responses_path, &provider.chat_completions_path, &provider.completions_path,
            &provider.embeddings_path, &provider.audio_transcriptions_path, &provider.audio_speech_path,
            &provider.messages_path, &provider.search_path, &provider.fetch_path, &provider.research_path,
            &provider.balance_path],
    })
}

fn kiro_runtime_profile<'a>(
    provider: &'a ProviderConfigYaml,
    credential: Option<&'a ProviderCredentialYaml>,
) -> Option<&'a Value> {
    if credential
        .and_then(|c| c.auth_mode.as_deref())
        .or(provider.auth_mode.as_deref())
        != Some(crate::protocol::kiro_oauth::runtime::AUTH_MODE)
    {
        return None;
    }
    credential
        .and_then(|c| c.extra_body.get("profileArn"))
        .or_else(|| provider.extra_body.get("profileArn"))
}

fn regions(
    provider: &ProviderConfigYaml,
    credential: Option<&ProviderCredentialYaml>,
) -> BTreeSet<String> {
    // Exact keys override at the real merge owner; only then normalize region aliases.
    let mut merged: BTreeMap<_, _> = provider
        .extra_body
        .iter()
        .filter(|(key, _)| is_region(key))
        .collect();
    if let Some(credential) = credential {
        merged.extend(
            credential
                .extra_body
                .iter()
                .filter(|(key, _)| is_region(key)),
        );
    }
    merged.values().map(|value| value.to_string()).collect()
}

fn is_region(key: &str) -> bool {
    let key: String = key
        .chars()
        .filter(|ch| !matches!(ch, '_' | '-'))
        .map(|ch| ch.to_ascii_lowercase())
        .collect();
    matches!(key.as_str(), "awsregion" | "region")
}
