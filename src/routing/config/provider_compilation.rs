//! Provider credential pools and initial OAuth runtime state.

use super::*;

/// Compile a `ProviderConfigYaml` into a `CompiledProvider`.
///
/// If a `preset` name is given, we look it up in the built-in registry and
/// call `compile_provider_account`.  Otherwise we build the payload directly
/// from the YAML fields.
///
/// When `credentials` is non-empty, builds a credential pool where each
/// credential has a fully-merged payload.  The provider's `payload` field
/// contains the base (first credential or provider-level key), and
/// `credential_pool` stores all credential payloads for round-robin selection.
pub(super) fn compile_provider(cfg: ProviderConfigYaml) -> Result<CompiledProvider, anyhow::Error> {
    if let Some(policy) = &cfg.request_wait {
        policy.validate()?;
    }
    let label = cfg.label.clone().unwrap_or_else(|| cfg.id.clone());

    // Build the base payload (used as-is when no credential pool exists).
    let mut payload = build_base_payload(
        &cfg, None, None, None, None, None, None, None, None, None, None, None, None, None,
    )?;
    if let Some(health) = &cfg.site_health {
        health.validate(&payload.base_url)?;
    }
    let site_health = cfg
        .site_health
        .as_ref()
        .map(|c| c.compile(&payload.base_url));
    call_flow::configure(
        &mut payload,
        cfg.call_flow,
        cfg.request_client_profile,
        None,
        cfg.codex_basispoints_enabled,
        None,
    )?;

    // Derive protocol_family from adapter name if not explicitly set.
    let protocol_family = infer_protocol_family(
        cfg.protocol_family.as_deref(),
        &payload.adapter,
        cfg.protocol_profile
            .as_deref()
            .or(cfg.preset.as_deref())
            .or(cfg.label.as_deref())
            .or(Some(cfg.id.as_str())),
        Some(payload.base_url.as_str()),
    );
    let protocol_profile = cfg
        .protocol_profile
        .as_deref()
        .map(canonicalize_protocol_profile_key)
        .unwrap_or_else(|| {
            cfg.preset
                .as_deref()
                .map(default_protocol_profile_for_preset)
                .map(str::to_string)
                .unwrap_or_else(|| {
                    infer_protocol_profile(
                        &payload.adapter,
                        cfg.preset
                            .as_deref()
                            .or(cfg.label.as_deref())
                            .or(Some(cfg.id.as_str())),
                        Some(payload.base_url.as_str()),
                    )
                })
        });

    // Build credential pool if `credentials` array is present.
    let credential_pool = if cfg.credentials.is_empty() {
        Vec::new()
    } else {
        cfg.credentials
            .iter()
            .enumerate()
            .map(|(idx, cred)| {
                let cred_id = effective_credential_id(&cfg.id, idx, cred).into_owned();

                let mut cred_payload = build_base_payload(
                    &cfg,
                    cred.base_url.as_deref(),
                    cred.api_key.as_deref(),
                    cred.auth_token.as_deref(),
                    if cred.headers.is_empty() {
                        None
                    } else {
                        Some(&cred.headers)
                    },
                    if cred.extra_body.is_empty() {
                        None
                    } else {
                        Some(&cred.extra_body)
                    },
                    cred.session_auth.as_ref(),
                    cred.keepalive.as_ref(),
                    cred.expires_at.as_deref(),
                    cred.runtime_state_object_key.as_deref(),
                    cred.account_name.as_deref(),
                    cred.execution_mode,
                    cred.endpoint_execution_modes.as_ref(),
                    Some(&cred_id),
                )?;

                if let Some(discovery) = &cred.discovery {
                    discovery.apply(&mut cred_payload)?;
                }
                let mut supported_models = cred
                    .discovery
                    .as_ref()
                    .map(|d| d.models.clone())
                    .unwrap_or_else(|| cred.supported_models.clone());
                if let Some(catalog) = &cred.model_catalog {
                    catalog.validate(cfg.preset.as_deref(), &cred_payload.base_url)?;
                    if cred.model_selection.is_none() {
                        catalog.append_models_to(&mut supported_models);
                    }
                }
                if let Some(selection) = &cred.model_selection {
                    supported_models = selection.models()?;
                }
                call_flow::configure(
                    &mut cred_payload,
                    cfg.call_flow,
                    cfg.request_client_profile,
                    cred.request_client_profile,
                    cfg.codex_basispoints_enabled,
                    cred.codex_basispoints_enabled,
                )?;

                // Build OAuth refresh config if refresh_token is provided.
                let refresh_config = TokenRefreshState::from_credential(cred).map(Arc::new);

                Ok(CompiledCredential {
                    discovery_required: cred.discovery_job.is_some() && cred.discovery.is_none(),
                    discovery: cred.discovery.clone(),
                    id: cred_id,
                    payload: cred_payload,
                    enabled: cred.enabled.unwrap_or(true),
                    supported_models,
                    explicit_model_selection: cred.model_selection.is_some(),
                    scheduled_probe_enabled: cred.scheduled_probe_enabled,
                    scheduled_probe_interval_minutes: cred
                        .scheduled_probe_interval_minutes
                        .unwrap_or(60)
                        .clamp(1, 10_080),
                    refresh_config,
                })
            })
            .collect::<Result<Vec<_>, anyhow::Error>>()?
    };

    let mut supported_models = cfg.supported_models;
    for credential in &cfg.credentials {
        if let Some(selection) = &credential.model_selection {
            for model in selection.models()? {
                if !supported_models.contains(&model) {
                    supported_models.push(model);
                }
            }
        } else if let Some(catalog) = &credential.model_catalog {
            catalog.append_models_to(&mut supported_models);
        }
    }
    let mut provider = CompiledProvider {
        explicit_credentials_only: cfg.explicit_credentials_only,
        site_health,
        credential_pool_auth_policy: cfg.credential_pool_auth_policy,
        id: cfg.id,
        label,
        payload,
        protocol_family: canonicalize_protocol_family_key(&protocol_family),
        protocol_profile,
        supported_models,
        model_map: cfg.model_map,
        model_map_targets: cfg.model_map_targets,
        scheduled_probe_enabled: cfg.scheduled_probe_enabled,
        scheduled_probe_interval_minutes: cfg
            .scheduled_probe_interval_minutes
            .unwrap_or(60)
            .clamp(1, 10_080),
        credential_pool,
        credential_counter: std::sync::Arc::new(AtomicUsize::new(0)),
    };
    super::search_tools::add_operation_mappings(&mut provider);
    Ok(provider)
}
