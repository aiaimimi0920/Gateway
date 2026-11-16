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
    copilot::validate_declaration(&cfg)?;
    if cfg.credentials.iter().any(|credential| {
        credential
            .token_expires_in_secs
            .is_some_and(|seconds| !token_expiry_supported(seconds))
    }) {
        anyhow::bail!("token_expires_in_secs cannot be represented safely by the runtime clock");
    }
    qwen_workspace::validate_declaration(&cfg)?;
    crate::protocol::agnes::validate_declaration(&cfg)?;
    crate::protocol::sensenova::validate_declaration(&cfg)?;
    crate::protocol::commandcode::validate_declaration(&cfg)?;
    crate::protocol::sap_aicore::validate_declaration(&cfg)?;
    crate::protocol::minimax::validate_declaration(&cfg)?;
    luma_mcp::validate_declaration(&cfg)?;
    luma_agents::validate_declaration(&cfg)?;
    grok_oauth::validate_declaration(&cfg)?;
    kiro_oauth::validate_declaration(&cfg)?;
    kiro_api_key::validate_declaration(&cfg)?;
    vertex_express::validate_declaration(&cfg)?;
    vertex_oauth::validate_declaration(&cfg)?;
    dim_oauth::validate_declaration(&cfg)?;
    nous_portal::validate_declaration(&cfg)?;
    lobsterai::validate_declaration(&cfg)?;
    perplexity_web::validate_declaration(&cfg)?;
    freebuff_session::validate_declaration(&cfg)?;
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
        !qwen_workspace::is_template(&cfg),
    )?;
    qwen_workspace::validate_parent(&cfg, &payload)?;
    cloudflare_workers_ai::validate_final(&cfg, &payload)?;
    crate::protocol::agnes::validate(&payload, payload.canonical_adapter())?;
    crate::protocol::sensenova::validate(&payload, payload.canonical_adapter())?;
    crate::protocol::commandcode::validate(&payload, payload.canonical_adapter())?;
    crate::protocol::sap_aicore::validate(&payload, payload.canonical_adapter())?;
    crate::protocol::minimax::validate(&payload, payload.canonical_adapter())?;
    luma_mcp::validate_final(&cfg, &payload)?;
    luma_agents::validate_final(&cfg, &payload)?;
    grok_oauth::validate_final(&cfg, &payload)?;
    kiro_api_key::validate_final(&cfg, &payload)?;
    vertex_express::validate_final(&cfg, &payload)?;
    perplexity_web::validate_final(&cfg, &payload)?;
    // An explicit empty pool is only a template; its base is not an executable account.
    if cfg.uses_provider_default_account() {
        crate::upstream::bedrock_auth::validate_configuration(&payload)?;
    }

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

                cred_payload.auth_mode = cred.auth_mode.clone().or(cred_payload.auth_mode);

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
                    crate::protocol::commandcode::catalog::configure(&mut cred_payload, catalog);
                    if cred.model_selection.is_none() {
                        catalog.append_models_to(&mut supported_models);
                    }
                }
                call_flow::configure(
                    &mut cred_payload,
                    cfg.call_flow,
                    cfg.request_client_profile,
                    cred.request_client_profile,
                    cfg.codex_basispoints_enabled,
                    cred.codex_basispoints_enabled,
                    true,
                )?;
                qwen_workspace::validate_final(&cfg, &cred_payload)?;
                cloudflare_workers_ai::validate_final(&cfg, &cred_payload)?;
                crate::protocol::agnes::validate(&cred_payload, cred_payload.canonical_adapter())?;
                crate::protocol::sap_aicore::validate(
                    &cred_payload,
                    cred_payload.canonical_adapter(),
                )?;
                crate::protocol::sensenova::validate(
                    &cred_payload,
                    cred_payload.canonical_adapter(),
                )?;
                crate::protocol::commandcode::validate(
                    &cred_payload,
                    cred_payload.canonical_adapter(),
                )?;
                crate::protocol::minimax::validate(
                    &cred_payload,
                    cred_payload.canonical_adapter(),
                )?;
                luma_mcp::validate_final(&cfg, &cred_payload)?;
                luma_agents::validate_final(&cfg, &cred_payload)?;
                grok_oauth::validate_final(&cfg, &cred_payload)?;
                perplexity_web::validate_final(&cfg, &cred_payload)?;
                kiro_oauth::configure(cred, &mut cred_payload)?;
                kiro_api_key::validate_final(&cfg, &cred_payload)?;
                vertex_express::validate_final(&cfg, &cred_payload)?;
                vertex_oauth::configure(cred, &mut cred_payload)?;
                dim_oauth::configure(cred, &cred_payload)?;
                nous_portal::configure(cred, &cred_payload)?;
                lobsterai::configure(cred, &mut cred_payload)?;
                freebuff_session::validate_final(&cfg, cred, &cred_payload)?;
                if let Some(selection) = &cred.model_selection {
                    supported_models = selection.models_for_payload(&cred_payload)?;
                }
                crate::upstream::bedrock_auth::validate_configuration(&cred_payload)?;

                // Build OAuth refresh config if refresh_token is provided.
                let refresh_config = TokenRefreshState::from_credential(cred).map(Arc::new);

                Ok(CompiledCredential {
                    dim_catalog: cred.dim_catalog.clone(),
                    discovery_required: cred.discovery_job.is_some() && cred.discovery.is_none(),
                    discovery: cred.discovery.clone(),
                    id: cred_id,
                    payload: cred_payload,
                    enabled: if copilot::is_provider(&cfg) {
                        copilot::credential_enabled(cred)
                    } else {
                        cred.enabled.unwrap_or(true)
                    },
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

    model_identity_validation::validate(&cfg, &payload, &credential_pool)?;
    let mut supported_models = cfg.supported_models;
    for (credential, compiled) in cfg.credentials.iter().zip(&credential_pool) {
        if credential.model_selection.is_some() {
            for model in &compiled.supported_models {
                if !supported_models.contains(model) {
                    supported_models.push(model.clone());
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
