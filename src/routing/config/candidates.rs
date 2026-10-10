//! Ordered provider resolution and constrained credential round-robin.

use super::*;

/// Candidate ordering and explicit route provenance from the same immutable snapshot.
pub struct CandidateResolution {
    pub candidates: Vec<RouteCandidate>,
    /// None means automatic discovery; Some(empty) means only disabled rules matched.
    pub explicit_provider_ids: Option<Vec<String>>,
}

pub(super) fn resolve_candidates_inner(
    guard: &RouteConfigInner,
    model: Option<&str>,
) -> Vec<RouteCandidate> {
    resolve_candidates_inner_with_account_filter(guard, model, None)
}

pub(super) fn resolve_candidates_inner_with_account_filter(
    guard: &RouteConfigInner,
    model: Option<&str>,
    allowed_account_ids: Option<&HashSet<String>>,
) -> Vec<RouteCandidate> {
    resolve_candidates_with_authorization(guard, model, allowed_account_ids).candidates
}

pub(super) fn resolve_candidates_with_authorization(
    guard: &RouteConfigInner,
    model: Option<&str>,
    allowed_account_ids: Option<&HashSet<String>>,
) -> CandidateResolution {
    let mut explicit_provider_ids = None;
    let provider_map: HashMap<&str, &CompiledProvider> = guard
        .providers
        .iter()
        .map(|provider| (provider.id.as_str(), provider))
        .collect();

    let resolved_alias = model.and_then(|model| resolve_alias_inner(guard, model));
    let alias_used = match (model, resolved_alias.as_deref()) {
        (Some(original), Some(resolved)) if resolved != original => Some(original),
        _ => None,
    };
    let resolved = match (resolved_alias.as_deref(), model) {
        (Some(target), _) => Some(target),
        (None, model) => model,
    };

    struct MatchedProvider<'a> {
        id: &'a str,
        priority: i32,
    }

    let matched: Vec<MatchedProvider> = match resolved {
        None => guard
            .providers
            .iter()
            .map(|provider| MatchedProvider {
                id: provider.id.as_str(),
                priority: 10,
            })
            .collect(),
        Some(model) => {
            let mut matching: Vec<&ModelRoute> = guard
                .model_routes
                .iter()
                .filter(|route| glob_match(&route.pattern, model))
                .collect();

            if matching.is_empty() {
                let explicitly_supported: Vec<MatchedProvider> = guard
                    .providers
                    .iter()
                    .filter(|provider| model_mapping::provider_supports_model(provider, model))
                    .map(|provider| MatchedProvider {
                        id: provider.id.as_str(),
                        priority: 10,
                    })
                    .collect();

                if explicitly_supported.is_empty() {
                    guard
                        .providers
                        .iter()
                        .map(|provider| MatchedProvider {
                            id: provider.id.as_str(),
                            priority: 10,
                        })
                        .collect()
                } else {
                    explicitly_supported
                }
            } else {
                matching.sort_by(|left, right| right.priority.cmp(&left.priority));
                let mut seen = std::collections::HashSet::new();
                let mut result = Vec::new();
                for route in matching {
                    // A disabled rule contributes nothing, but it already
                    // counted as a match above, so the model stays off rather
                    // than falling back to every supporting provider.
                    if !route.enabled {
                        continue;
                    }
                    for id in &route.provider_ids {
                        if seen.insert(id.as_str()) {
                            result.push(MatchedProvider {
                                id: id.as_str(),
                                priority: route.priority,
                            });
                        }
                    }
                }
                explicit_provider_ids = Some(
                    result
                        .iter()
                        .map(|provider| provider.id.to_string())
                        .collect(),
                );
                result
            }
        }
    };

    let candidates = matched
        .into_iter()
        .filter_map(|matched| {
            provider_map.get(matched.id).and_then(|provider| {
                provider_to_candidate(
                    provider,
                    resolved,
                    alias_used,
                    matched.priority,
                    allowed_account_ids,
                )
            })
        })
        .collect();
    CandidateResolution {
        candidates,
        explicit_provider_ids,
    }
}

/// Select a credential's payload from the pool using model-aware round-robin.
///
/// If credentials have `supported_models`, only those matching the requested
/// model are eligible. This handles providers like xfyun where different
/// appIds are authorized for different models.
///
/// If the pool is empty, returns the provider's base payload.
/// Select a credential from the pool that supports the requested model.
///
/// `original_model` is the user-facing name (e.g. "qwen3.5-35b-a3b").
/// `translated_model` is the model_map-translated upstream name (e.g. "astron-code-latest").
///
/// A credential matches if its `supported_models` contains EITHER name.
/// This allows credentials to list either user-facing or translated upstream
/// model names; both naming schemes work.
pub(super) fn select_credential(
    provider: &CompiledProvider,
    original_model: Option<&str>,
    translated_model: Option<&str>,
    allowed_account_ids: Option<&HashSet<String>>,
) -> Option<ProviderAccountPayload> {
    if provider.credential_pool.is_empty() {
        let default_account_id = provider_default_account_id(&provider.id);
        if allowed_account_ids.is_some_and(|allowed| !allowed.contains(&default_account_id)) {
            return None;
        }
        return Some(provider.payload.clone());
    }

    // Establish the runtime-available pool first. Group membership and enabled
    // state are hard constraints and must never be bypassed by model fallback.
    let available: Vec<usize> = provider
        .credential_pool
        .iter()
        .enumerate()
        .filter(|(_, credential)| credential.enabled && !credential.discovery_required)
        .filter(|(_, credential)| {
            allowed_account_ids.is_none_or(|allowed| allowed.contains(&credential.id))
        })
        .map(|(index, _)| index)
        .collect();
    if available.is_empty() {
        return None;
    }

    // Filter available credentials that support the requested model. Match
    // against both the original (user-facing) and translated (upstream) name.
    let model_eligible: Vec<usize> = available
        .iter()
        .copied()
        .filter(|index| {
            let cred = &provider.credential_pool[*index];
            // A discovered catalogue names upstream models, not arbitrary local aliases.
            if cred.discovery.is_some() {
                return translated_model.or(original_model).is_none_or(|model| {
                    cred.supported_models
                        .iter()
                        .any(|supported| supported == model)
                });
            }
            if cred.supported_models.is_empty() {
                true // no restriction = supports all provider models
            } else if original_model.is_none() && translated_model.is_none() {
                true // no model specified = any credential
            } else {
                // Match if credential supports either the original or translated name
                let matches_original = original_model
                    .map(|m| cred.supported_models.iter().any(|sm| sm == m))
                    .unwrap_or(false);
                let matches_translated = translated_model
                    .map(|m| cred.supported_models.iter().any(|sm| sm == m))
                    .unwrap_or(false);
                matches_original || matches_translated
            }
        })
        .collect();

    // Explicit group membership must not expose another account's model through
    // compatibility fallback when none of the authorized accounts supports it.
    if model_eligible.is_empty() && allowed_account_ids.is_some() {
        return None;
    }
    // Unrestricted legacy routing retains its model compatibility fallback.
    let legacy: Vec<usize> = available
        .into_iter()
        .filter(|index| provider.credential_pool[*index].discovery.is_none())
        .collect();
    let eligible = if model_eligible.is_empty() {
        &legacy
    } else {
        &model_eligible
    };
    if eligible.is_empty() {
        return None;
    }

    // Round-robin among eligible credentials
    let counter = provider.credential_counter.fetch_add(1, Ordering::Relaxed);
    let idx = eligible[counter % eligible.len()];
    Some(apply_token_override(&provider.credential_pool[idx]))
}

/// Apply OAuth token override if a refreshed access_token is available.
pub(super) fn apply_token_override(cred: &CompiledCredential) -> ProviderAccountPayload {
    let mut payload = cred.payload.clone();
    if let Some(ref config) = cred.refresh_config {
        let (override_key, expiry) = config.runtime_override();
        if let Some(override_key) = override_key {
            payload.api_key = override_key;
        }
        if let Some(expiry) = expiry {
            if let Some(session_auth) = payload.session_auth.as_mut() {
                session_auth.expires_at = Some(expiry);
            }
        }
    }
    payload
}

/// Convert a `CompiledProvider` to a `RouteCandidate`.
///
/// `model` is the resolved model name (after alias resolution) to send
/// upstream; `alias` is the original alias if one was used.
///
/// When the provider has a credential pool, the candidate's `payload` is set
/// to a round-robin-selected credential, but `provider_account_id` always
/// references the provider — one AIMD controller for the whole pool.
fn provider_to_candidate(
    p: &CompiledProvider,
    model: Option<&str>,
    alias: Option<&str>,
    priority: i32,
    allowed_account_ids: Option<&HashSet<String>>,
) -> Option<RouteCandidate> {
    // A group-scoped default account must not inherit compatibility fallback
    // for models outside its declared catalogue. Aliases are already resolved.
    if allowed_account_ids.is_some()
        && p.credential_pool.is_empty()
        && !p.supported_models.is_empty()
        && model.is_some_and(|model| !model_mapping::provider_supports_model(p, model))
    {
        return None;
    }
    // Select the payload — either base or a pooled credential.
    // Model-aware: only picks credentials authorized for the requested model.
    //
    // We pass BOTH the original (user-facing) and translated (upstream) model
    // names so credential matching works regardless of which naming scheme the
    // credential's `supported_models` list uses.
    //
    // Example 1 — xfyun-maas: user requests "hunyuan-mt-7b", model_map → "xophunyuan7bmt",
    //   credential lists "xophunyuan7bmt" → matches via translated name.
    let (upstream_model, selected_payload) =
        if let Some(model) = model.filter(|m| p.model_map_targets.contains_key(*m)) {
            let (mapped, payload) =
                model_mapping::select_mapped_payload(p, model, allowed_account_ids)?;
            (Some(mapped), payload)
        } else {
            let translated = model.and_then(|m| p.model_map.get(m).map(String::as_str));
            (
                model.map(|m| translated.unwrap_or(m).to_string()),
                select_credential(p, model, translated, allowed_account_ids)?,
            )
        };
    let resolved_execution_mode = selected_payload
        .resolve_execution_mode(crate::protocol::canonical::EndpointKind::ChatCompletions);
    let discovery = p
        .credential_pool
        .iter()
        .find(|c| Some(c.id.as_str()) == selected_payload.credential_id.as_deref())
        .and_then(|c| c.discovery.as_ref());
    let family = discovery.map(|d| d.family()).unwrap_or(&p.protocol_family);

    Some(RouteCandidate {
        provider_account_id: p.id.clone(),
        provider_credential_id: None,
        label: p.label.clone(),
        adapter: selected_payload.adapter.clone(),
        protocol_family: family.to_owned(),
        protocol_profile: discovery
            .map(|d| {
                match d.protocol {
                    crate::provider_discovery::DiscoveredProtocol::Messages => "anthropic_messages",
                    _ => "openai_compatible_generic",
                }
                .to_owned()
            })
            .unwrap_or_else(|| p.protocol_profile.clone()),
        supported_protocol_families: if discovery.is_some() {
            selected_payload
                .discovered_protocols
                .iter()
                .map(|c| c.protocol.family().to_owned())
                .collect()
        } else {
            surface_supported_wire_protocol_families(&selected_payload.adapter, family)
        },
        payload: selected_payload,
        model_alias: alias.map(|s| s.to_string()),
        upstream_model,
        resolved_execution_mode,
        priority,
        weight: 100,
        failure_count: 0,
        cooldown_until: None,
        routing_score: None,
        routing_health_weight: None,
        routing_capacity_weight: None,
        routing_degraded: None,
        routing_breaker_open: None,
        routing_degradation_reasons: Vec::new(),
    })
}
