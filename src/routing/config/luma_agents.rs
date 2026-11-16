//! Preserve declared Agents identity across preset, account and flow merges; no generic discovery.
use super::{ProviderAccountPayload, ProviderConfigYaml, ProviderCredentialYaml};
use crate::protocol::{luma_agents as agents, registry};

/// The complete active document can identify a legacy Key; redacted drafts retain public identity.
pub(crate) fn owns(
    provider: &ProviderConfigYaml,
    credential: Option<&ProviderCredentialYaml>,
) -> bool {
    // MCP's separate declaration owner validates this management-only profile first.
    if super::luma_mcp::is_provider(provider) {
        return false;
    }
    provider.preset.as_deref() == Some(agents::PRESET_ID)
        || provider.adapter.as_deref() == Some(agents::ADAPTER)
        || provider
            .protocol_profile
            .as_deref()
            .is_some_and(|p| registry::canonicalize_protocol_profile_key(p) == agents::PROFILE)
        || agents::owns_base(
            credential
                .and_then(|c| c.base_url.as_deref())
                .unwrap_or(&provider.base_url),
        )
        || credential
            .and_then(|c| c.api_key.as_deref())
            .unwrap_or(&provider.api_key)
            .starts_with("luma-api-")
}
fn owns_any(cfg: &ProviderConfigYaml) -> bool {
    owns(cfg, None) || cfg.credentials.iter().any(|c| owns(cfg, Some(c)))
}
fn invalid() -> crate::error::GatewayError {
    agents::input(
        "luma_agents_configuration",
        "Luma Agents 配置必须保持独立静态 API Key 产品；不能混入 Web、OAuth、目录或其他执行参数。",
    )
}
pub(super) fn validate_declaration(cfg: &ProviderConfigYaml) -> anyhow::Result<()> {
    if !owns_any(cfg) {
        return Ok(());
    }
    crate::implementation_lines::ensure_protocol_profile_compiled(agents::PROFILE)?;
    let models = cfg
        .supported_models
        .iter()
        .chain(cfg.model_map.values())
        .chain(cfg.model_map_targets.values().flatten())
        .chain(cfg.default_model.iter());
    if cfg
        .preset
        .as_deref()
        .is_some_and(|p| p != agents::PRESET_ID)
        || cfg
            .protocol_profile
            .as_deref()
            .is_some_and(|p| registry::canonicalize_protocol_profile_key(p) != agents::PROFILE)
        || cfg
            .protocol_family
            .as_deref()
            .is_some_and(|p| registry::canonicalize_protocol_family_key(p) != agents::FAMILY)
        || cfg.codex_basispoints_enabled
        || models.into_iter().any(|m| !agents::valid_model(m))
        || cfg.credentials.iter().any(|c| {
            c.refresh_token.is_some()
                || c.refresh_endpoint.is_some()
                || c.refresh_client_id.is_some()
                || c.token_expires_in_secs.is_some()
                || c.grok_oauth.is_some()
                || c.kiro_oauth.is_some()
                || c.vertex_oauth.is_some()
                || c.accio_sgcookie.is_some()
                || c.discovery.is_some()
                || c.discovery_job.is_some()
                || c.model_catalog.is_some()
                || c.codex_basispoints_enabled == Some(true)
                || c.supported_models.iter().any(|m| !agents::valid_model(m))
                || c.model_selection.as_ref().is_some_and(|s| {
                    s.manual
                        .iter()
                        .chain(&s.fetched)
                        .any(|m| !agents::valid_model(m))
                })
        })
    {
        return Err(invalid().into());
    }
    Ok(())
}
pub(super) fn validate_final(
    cfg: &ProviderConfigYaml,
    payload: &ProviderAccountPayload,
) -> anyhow::Result<()> {
    if !owns_any(cfg) {
        return Ok(());
    }
    agents::validate(payload, payload.canonical_adapter())?;
    use crate::{
        protocol::{
            call_flow::{CallAdapter, FingerprintAdapter, SpecialAdapter},
            request_profile::RequestClientProfile,
        },
        routing::candidate::ProviderExecutionMode,
    };
    if payload.adapter != agents::ADAPTER
        || !payload.api_key.is_empty() && !agents::valid_key(&payload.api_key)
        || !payload.headers.is_empty()
        || payload.extra_body.as_ref().is_some_and(|b| !b.is_empty())
        || payload.keepalive.is_some()
        || payload.runtime_state_object_key.is_some()
        || payload
            .auth_header_name
            .as_deref()
            .is_some_and(|h| !h.eq_ignore_ascii_case("authorization"))
        || !matches!(
            payload.request_client_profile,
            None | Some(RequestClientProfile::Auto)
        )
        || payload
            .execution_mode
            .is_some_and(|m| m != ProviderExecutionMode::DirectHttp)
        || payload
            .endpoint_execution_modes
            .as_ref()
            .is_some_and(|m| m.values().any(|v| *v != ProviderExecutionMode::DirectHttp))
        || payload.call_flow.is_some_and(|f| {
            f.call_adapter != CallAdapter::Auto
                || f.fingerprint_adapter != FingerprintAdapter::Auto
                || f.special_adapter != SpecialAdapter::Auto
        })
        || [
            &payload.responses_path,
            &payload.chat_completions_path,
            &payload.messages_path,
            &payload.completions_path,
            &payload.embeddings_path,
            &payload.audio_transcriptions_path,
            &payload.audio_speech_path,
            &payload.search_path,
            &payload.fetch_path,
            &payload.research_path,
            &payload.balance_path,
        ]
        .iter()
        .any(|p| p.is_some())
    {
        return Err(invalid().into());
    }
    crate::protocol::call_flow::prepare(payload)?;
    Ok(())
}
