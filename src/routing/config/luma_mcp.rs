//! Credential-only MCP storage; disabled pools cannot become inference or background-probe accounts.
use super::{ProviderAccountPayload, ProviderConfigYaml, ProviderCredentialYaml};
use crate::protocol::{luma_mcp::boundary as product, luma_oauth::RESOURCE};

pub(crate) fn is_provider(cfg: &ProviderConfigYaml) -> bool {
    cfg.adapter.as_deref() == Some(product::ADAPTER)
}
pub(crate) fn owns(cfg: &ProviderConfigYaml) -> bool {
    is_provider(cfg)
        || cfg.auth_mode.as_deref() == Some(product::AUTH_MODE)
        || cfg.protocol_profile.as_deref() == Some(product::PROFILE)
        || cfg.protocol_family.as_deref() == Some(product::FAMILY)
        || product::owns_base(&cfg.base_url)
        || cfg.credentials.iter().any(|c| {
            c.luma_oauth.is_some()
                || c.auth_mode.as_deref() == Some(product::AUTH_MODE)
                || c.base_url.as_deref().is_some_and(product::owns_base)
        })
}

pub(crate) fn validate_declaration(cfg: &ProviderConfigYaml) -> anyhow::Result<()> {
    if !owns(cfg) {
        return Ok(());
    }
    if !is_provider(cfg)
        || cfg.preset.is_some()
        || cfg.base_url != RESOURCE
        || cfg.auth_mode.as_deref() != Some(product::AUTH_MODE)
        || cfg.protocol_profile.as_deref() != Some(product::PROFILE)
        || cfg.protocol_family.as_deref() != Some(product::FAMILY)
        || !cfg.explicit_credentials_only
        || !cfg.api_key.is_empty()
        || cfg.call_flow.is_some()
        || cfg.request_client_profile.is_some()
        || cfg.codex_basispoints_enabled
        || cfg.site_health.is_some()
        || cfg.scheduled_probe_enabled
        || cfg.auto_refill_enabled
        || cfg.auto_prune_enabled
        || cfg.credential_automation_driver_id.is_some()
        || !cfg.supported_models.is_empty()
        || cfg.default_model.is_some()
        || !cfg.model_map.is_empty()
        || !cfg.model_map_targets.is_empty()
        || !cfg.test_plans.is_empty()
        || cfg.test_policy.is_some()
        || !cfg.subpool_test_policies.is_empty()
        || cfg.default_account_test_policy.is_some()
        || cfg.credentials.iter().any(invalid_credential)
    {
        return Err(product::invalid().into());
    }
    Ok(())
}

fn invalid_credential(c: &ProviderCredentialYaml) -> bool {
    c.luma_oauth.is_none()
        || c.copilot_github_token.is_some()
        || c.test_policy.is_some()
        || c.enabled != Some(false)
        || c.scheduled_probe_enabled
        || c.api_key
            .as_deref()
            .is_some_and(|v| v.starts_with("luma-api-"))
        || c.auth_mode
            .as_deref()
            .is_some_and(|v| v != product::AUTH_MODE)
        || c.base_url.as_deref().is_some_and(|v| v != RESOURCE)
        || c.request_client_profile.is_some()
        || c.codex_basispoints_enabled.is_some()
        || !c.supported_models.is_empty()
        || c.model_selection.is_some()
        || c.discovery.is_some()
        || c.discovery_job.is_some()
        || c.model_catalog.is_some()
        || c.grok_oauth.is_some()
        || c.kiro_oauth.is_some()
        || c.quick_oauth.is_some()
        || c.vertex_oauth.is_some()
        || c.dim_oauth.is_some()
        || c.dim_catalog.is_some()
        || c.nous_portal.is_some()
        || c.lobsterai.is_some()
        || c.lobsterai_catalog.is_some()
        || c.freebuff_session.is_some()
        || c.accio_sgcookie.is_some()
        // Cleared secrets may leave readable metadata; only a present refresh requires offline context.
        || c.refresh_token.as_deref().is_some_and(|token| {
            !c.luma_oauth.as_ref().is_some_and(|context| context.offline_access())
                || !crate::protocol::luma_oauth::valid_refresh(token)
        })
        || c.refresh_endpoint.is_some()
        || c.refresh_client_id.is_some()
        || c.token_expires_in_secs.is_some()
}

pub(super) fn validate_final(
    cfg: &ProviderConfigYaml,
    p: &ProviderAccountPayload,
) -> anyhow::Result<()> {
    if !owns(cfg) {
        return Ok(());
    }
    if p.adapter != product::ADAPTER
        || p.base_url != RESOURCE
        || p.auth_mode.as_deref() != Some(product::AUTH_MODE)
        || p.auth_token.is_some()
        || p.session_auth.is_some()
        || p.keepalive.is_some()
        || !p.headers.is_empty()
        || p.extra_body.as_ref().is_some_and(|v| !v.is_empty())
        || p.runtime_state_object_key.is_some()
        || p.expires_at.is_some()
        || p.execution_mode.is_some()
        || p.endpoint_execution_modes.is_some()
        || p.call_flow.is_some()
        || p.request_client_profile.is_some()
        || p.auth_header_name.is_some()
        || !p.discovered_protocols.is_empty()
        || [
            &p.responses_path,
            &p.chat_completions_path,
            &p.messages_path,
            &p.completions_path,
            &p.embeddings_path,
            &p.audio_transcriptions_path,
            &p.audio_speech_path,
            &p.search_path,
            &p.fetch_path,
            &p.research_path,
            &p.balance_path,
        ]
        .iter()
        .any(|v| v.is_some())
    {
        return Err(product::invalid().into());
    }
    Ok(())
}
