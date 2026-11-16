//! Preset-anchored configuration checks; only non-executable parents may be unbound templates.
use super::{ProviderAccountPayload, ProviderConfigYaml, ProviderExecutionMode};
use crate::{
    protocol::{
        call_flow::{CallAdapter, FingerprintAdapter, SpecialAdapter},
        qwen::workspace_api,
    },
    provider_discovery::model_catalog::qwen_endpoint::QwenEndpoint,
};

pub(super) fn owns(cfg: &ProviderConfigYaml) -> bool {
    cfg.preset.as_deref() == Some("qwen-official-api")
}
pub(super) fn is_template(cfg: &ProviderConfigYaml) -> bool {
    owns(cfg) && !cfg.uses_provider_default_account()
}
fn invalid() -> anyhow::Error {
    anyhow::anyhow!("Invalid ordinary Qwen workspace API configuration.")
}

pub(super) fn validate_declaration(cfg: &ProviderConfigYaml) -> anyhow::Result<()> {
    if !owns(cfg) {
        return Ok(());
    }
    let bad = cfg
        .protocol_profile
        .as_deref()
        .is_some_and(|p| super::canonicalize_protocol_profile_key(p) != workspace_api::PROFILE)
        || cfg.protocol_family.as_deref().is_some_and(|p| {
            super::canonicalize_protocol_family_key(p)
                != crate::protocol::registry::OPENAI_CHAT_FAMILY
        })
        || cfg
            .adapter
            .as_deref()
            .is_some_and(|a| super::canonicalize_adapter_name(a) != "openai_compatible")
        || !matches!(
            cfg.auth_mode.as_deref(),
            None | Some("bearer") | Some(workspace_api::AUTH_MODE)
        )
        || !cfg.base_url.is_empty() && QwenEndpoint::parse(&cfg.base_url).is_none()
        || cfg.codex_basispoints_enabled
        || cfg.session_auth.is_some()
        || cfg.keepalive.is_some()
        || cfg.auth_token.is_some()
        || cfg
            .execution_mode
            .is_some_and(|m| m != ProviderExecutionMode::DirectHttp)
        || cfg.endpoint_execution_modes.as_ref().is_some_and(|modes| {
            modes
                .values()
                .any(|m| *m != ProviderExecutionMode::DirectHttp)
        })
        || cfg.headers.keys().any(|h| {
            matches!(
                h.to_ascii_lowercase().as_str(),
                "authorization" | "cookie" | "x-api-key" | "api-key" | "host"
            )
        })
        || [
            &cfg.responses_path,
            &cfg.messages_path,
            &cfg.completions_path,
            &cfg.embeddings_path,
            &cfg.audio_transcriptions_path,
            &cfg.audio_speech_path,
            &cfg.search_path,
            &cfg.fetch_path,
            &cfg.research_path,
            &cfg.balance_path,
        ]
        .iter()
        .any(|p| p.is_some())
        || cfg
            .chat_completions_path
            .as_deref()
            .is_some_and(|p| p != "/chat/completions")
        || cfg.call_flow.is_some_and(|f| {
            !matches!(f.call_adapter, CallAdapter::Auto | CallAdapter::Openai)
                || !matches!(
                    f.special_adapter,
                    SpecialAdapter::Auto | SpecialAdapter::None
                )
                || matches!(
                    f.fingerprint_adapter,
                    FingerprintAdapter::Session | FingerprintAdapter::Antigravity
                )
        });
    if bad {
        return Err(invalid());
    }
    for credential in &cfg.credentials {
        // These raw settings create background work but are not present in the inference payload.
        if credential.discovery.is_some()
            || credential.discovery_job.is_some()
            || credential.refresh_token.is_some()
            || credential.refresh_endpoint.is_some()
            || credential.refresh_client_id.is_some()
            || credential.token_expires_in_secs.is_some()
            || credential.codex_basispoints_enabled == Some(true)
        {
            return Err(invalid());
        }
    }
    Ok(())
}

pub(super) fn validate_parent(
    cfg: &ProviderConfigYaml,
    payload: &ProviderAccountPayload,
) -> anyhow::Result<()> {
    if !owns(cfg) {
        return Ok(());
    }
    // Empty public target is legal only for an abstract parent, never for an executable default.
    if payload.base_url.is_empty() && is_template(cfg) {
        return Ok(());
    }
    validate_final(cfg, payload)
}

pub(super) fn validate_final(
    cfg: &ProviderConfigYaml,
    payload: &ProviderAccountPayload,
) -> anyhow::Result<()> {
    if !owns(cfg) {
        return Ok(());
    }
    // The original preset is authoritative even if merged auth/base would no longer claim ownership.
    if QwenEndpoint::parse(&payload.base_url).is_none() {
        return Err(invalid());
    }
    crate::protocol::call_flow::prepare(payload)?;
    workspace_api::validate(payload, payload.canonical_adapter())?;
    // Clear/missing material remains storable for normal secret operations. It cannot send:
    // the unchanged runtime validate_key rejects an empty Key before network I/O.
    if !payload.api_key.is_empty() {
        workspace_api::validate_key(payload)?;
    }
    Ok(())
}
