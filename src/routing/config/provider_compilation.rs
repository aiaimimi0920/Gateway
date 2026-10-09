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
    let label = cfg.label.clone().unwrap_or_else(|| cfg.id.clone());

    // Build the base payload (used as-is when no credential pool exists).
    let payload = build_base_payload(
        &cfg, None, None, None, None, None, None, None, None, None, None, None, None, None,
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

                // Build OAuth refresh config if refresh_token is provided.
                let refresh_config = match (&cred.refresh_token, &cred.refresh_endpoint) {
                    (Some(rt), Some(ep)) => {
                        let expires_in_secs = cred.token_expires_in_secs.unwrap_or(21600);
                        let duration = std::time::Duration::from_secs(expires_in_secs);
                        let expires_at = std::time::Instant::now()
                            .checked_add(duration)
                            .ok_or_else(|| {
                                anyhow::anyhow!(
                                    "token_expires_in_secs cannot be represented by Instant"
                                )
                            })?;
                        let expires_at_system = std::time::SystemTime::now()
                            .checked_add(duration)
                            .ok_or_else(|| {
                                anyhow::anyhow!(
                                    "token_expires_in_secs cannot be represented by SystemTime"
                                )
                            })?;
                        Some(Arc::new(TokenRefreshState {
                            refresh_token: parking_lot::Mutex::new(rt.clone()),
                            refresh_endpoint: ep.clone(),
                            client_id: cred.refresh_client_id.clone().unwrap_or_default(),
                            expires_in_secs,
                            expires_at: parking_lot::Mutex::new(expires_at),
                            expires_at_iso: parking_lot::Mutex::new(Some(
                                system_time_to_rfc3339_millis(expires_at_system),
                            )),
                            api_key_override: parking_lot::Mutex::new(None),
                            lifecycle: parking_lot::Mutex::new(RefreshLifecycle {
                                active: true,
                                generation: 0,
                            }),
                        }))
                    }
                    _ => None,
                };

                Ok(CompiledCredential {
                    discovery: cred.discovery.clone(),
                    id: cred_id,
                    payload: cred_payload,
                    enabled: cred.enabled.unwrap_or(true),
                    supported_models: cred
                        .discovery
                        .as_ref()
                        .map(|d| d.models.clone())
                        .unwrap_or_else(|| cred.supported_models.clone()),
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

    Ok(CompiledProvider {
        id: cfg.id,
        label,
        payload,
        protocol_family: canonicalize_protocol_family_key(&protocol_family),
        protocol_profile,
        supported_models: cfg.supported_models,
        model_map: cfg.model_map,
        model_map_targets: cfg.model_map_targets,
        scheduled_probe_enabled: cfg.scheduled_probe_enabled,
        scheduled_probe_interval_minutes: cfg
            .scheduled_probe_interval_minutes
            .unwrap_or(60)
            .clamp(1, 10_080),
        credential_pool,
        credential_counter: std::sync::Arc::new(AtomicUsize::new(0)),
    })
}
