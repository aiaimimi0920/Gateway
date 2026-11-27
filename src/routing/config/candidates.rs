//! Ordered provider resolution and constrained credential round-robin.

use super::*;
mod diagnostics;
mod fallback;
pub use diagnostics::RejectedCandidate;
#[cfg(test)]
mod fallback_tests;

/// Candidate ordering and explicit route provenance from the same immutable snapshot.
pub struct CandidateResolution {
    pub candidates: Vec<RouteCandidate>,
    /// None means automatic discovery; Some(empty) means only disabled rules matched.
    pub explicit_provider_ids: Option<Vec<String>>,
    pub rejected_candidates: Vec<RejectedCandidate>,
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
    resolve_candidates_filtered(guard, model, allowed_account_ids, None, false, None)
}

pub(super) type Admission<'a> = Option<&'a dyn Fn(&str, &str, &str) -> bool>;

pub(super) fn resolve_candidates_filtered(
    guard: &RouteConfigInner,
    model: Option<&str>,
    allowed_account_ids: Option<&HashSet<String>>,
    admission: Admission<'_>,
    authorized_group_fallback: bool,
    preferred: Option<&(String, String)>,
) -> CandidateResolution {
    let mut explicit_provider_ids: Option<Vec<String>> = None;
    let provider_map: HashMap<&str, &CompiledProvider> = guard
        .providers
        .iter()
        .map(|provider| (provider.id.as_str(), provider))
        .collect();

    let resolved_alias = model.and_then(|model| resolve_alias_inner(guard, model));
    if model
        .into_iter()
        .chain(resolved_alias.as_deref())
        .any(cursor::local_auto::reserved)
    {
        return CandidateResolution {
            candidates: Vec::new(),
            explicit_provider_ids: Some(Vec::new()),
            rejected_candidates: Vec::new(),
        };
    }
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

    let mut matched: Vec<MatchedProvider> = match resolved {
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

    // Runtime group membership is an explicit account allow-list. Keep the configured
    // providers preferred, but retain authorized alternatives when those are disabled
    // or reject admission. A disabled model rule still disables the model entirely.
    if authorized_group_fallback
        && admission.is_some()
        && allowed_account_ids.is_some()
        && explicit_provider_ids
            .as_ref()
            .is_some_and(|ids| !ids.is_empty())
    {
        for provider in &guard.providers {
            if !matched.iter().any(|entry| entry.id == provider.id)
                && resolved
                    .is_some_and(|model| model_mapping::provider_supports_model(provider, model))
            {
                matched.push(MatchedProvider {
                    id: &provider.id,
                    priority: 0,
                });
                explicit_provider_ids
                    .as_mut()
                    .unwrap()
                    .push(provider.id.clone());
            }
        }
    }
    let mut rejected_candidates = Vec::new();
    let candidates: Vec<RouteCandidate> = matched
        .into_iter()
        .flat_map(|matched| {
            provider_map
                .get(matched.id)
                .filter(|provider| search_tools::allows_model(provider, resolved))
                .map_or_else(Vec::new, |provider| {
                    diagnostics::collect(
                        &mut rejected_candidates,
                        provider,
                        resolved,
                        allowed_account_ids,
                        admission,
                    );
                    fallback::provider_candidates(
                        provider,
                        resolved,
                        alias_used,
                        matched.priority,
                        allowed_account_ids,
                        admission,
                        preferred
                            .filter(|(id, _)| id == &provider.id)
                            .map(|(_, credential)| credential.as_str()),
                    )
                })
        })
        .collect();
    diagnostics::remove_admitted(&mut rejected_candidates, &candidates);
    if (authorized_group_fallback && admission.is_some() && allowed_account_ids.is_some())
        || resolved.is_some_and(search_tools::is_generic)
    {
        // Concrete membership or an explicit tool-pool request authorizes fallback
        // only among these already-filtered operation/account candidates.
        let mut ids = Vec::new();
        for candidate in &candidates {
            if !ids.contains(&candidate.provider_account_id) {
                ids.push(candidate.provider_account_id.clone());
            }
        }
        explicit_provider_ids = Some(ids);
    }
    CandidateResolution {
        candidates,
        explicit_provider_ids,
        rejected_candidates,
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
#[cfg(test)]
pub(super) fn select_credential(
    provider: &CompiledProvider,
    original_model: Option<&str>,
    translated_model: Option<&str>,
    allowed_account_ids: Option<&HashSet<String>>,
) -> Option<ProviderAccountPayload> {
    select_credential_filtered(
        provider,
        original_model,
        translated_model,
        allowed_account_ids,
        None,
        true,
    )
}

fn select_credential_filtered(
    provider: &CompiledProvider,
    original_model: Option<&str>,
    translated_model: Option<&str>,
    allowed_account_ids: Option<&HashSet<String>>,
    admission: Admission<'_>,
    rotate: bool,
) -> Option<ProviderAccountPayload> {
    if provider.uses_provider_default_account() {
        if !provider.credential_pool_auth_policy.is_any() {
            return None; // Base payloads have no typed refresh owner: unknown fails closed.
        }
        let default_account_id = provider_default_account_id(&provider.id);
        if allowed_account_ids.is_some_and(|allowed| !allowed.contains(&default_account_id)) {
            return None;
        }
        if !crate::protocol::vercel_free::account_allows(
            &provider.payload,
            translated_model.or(original_model),
        ) || !admits(
            admission,
            &provider.id,
            &default_account_id,
            translated_model.or(original_model),
        ) {
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
        .filter(|(_, c)| {
            crate::protocol::vercel_free::account_allows(
                &c.payload,
                translated_model.or(original_model),
            )
        })
        .filter(|(_, credential)| {
            credential
                .refresh_config
                .as_ref()
                .is_none_or(|owner| owner.is_routing_active())
        })
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
            credential_supports_model(cred, original_model, translated_model)
                && provider.credential_pool_auth_policy.allows(cred)
        })
        .collect();

    // Explicit group membership must not expose another account's model through
    // compatibility fallback when none of the authorized accounts supports it.
    if model_eligible.is_empty()
        && (allowed_account_ids.is_some() || !provider.credential_pool_auth_policy.is_any())
    {
        return None;
    }
    // Unrestricted legacy routing retains its model compatibility fallback.
    let legacy: Vec<usize> = available
        .into_iter()
        .filter(|index| {
            provider.credential_pool[*index].discovery.is_none()
                && !provider.credential_pool[*index].explicit_model_selection
        })
        .collect();
    let eligible: Vec<_> = (if model_eligible.is_empty() {
        &legacy
    } else {
        &model_eligible
    })
    .iter()
    .copied()
    .filter(|index| {
        admits(
            admission,
            &provider.id,
            &provider.credential_pool[*index].id,
            translated_model.or(original_model),
        )
    })
    .collect();
    if eligible.is_empty() {
        return None;
    }

    // Round-robin among eligible credentials
    let counter = if rotate {
        provider.credential_counter.fetch_add(1, Ordering::Relaxed)
    } else {
        provider.credential_counter.load(Ordering::Relaxed)
    };
    let idx = eligible[counter % eligible.len()];
    Some(apply_token_override(&provider.credential_pool[idx]))
}

pub(super) fn admits(
    admission: Admission<'_>,
    provider: &str,
    credential: &str,
    model: Option<&str>,
) -> bool {
    admission.is_none_or(|allow| allow(provider, credential, model.unwrap_or_default()))
}

pub(super) fn credential_supports_model(
    credential: &CompiledCredential,
    original: Option<&str>,
    translated: Option<&str>,
) -> bool {
    if !crate::protocol::vercel_free::account_allows(&credential.payload, translated.or(original)) {
        return false;
    }
    if credential.explicit_model_selection && credential.supported_models.is_empty() {
        return false;
    }
    // A discovered catalogue names upstream models, not arbitrary local aliases.
    if credential.discovery.is_some() {
        return translated.or(original).is_none_or(|model| {
            credential
                .supported_models
                .iter()
                .any(|supported| supported == model)
        });
    }
    credential.supported_models.is_empty()
        || (original.is_none() && translated.is_none())
        || original.into_iter().chain(translated).any(|model| {
            credential
                .supported_models
                .iter()
                .any(|supported| supported == model)
        })
}

/// Catalog check without consuming the round-robin counter or cloning secrets.
pub(super) fn has_model_eligible_oauth_credential(
    provider: &CompiledProvider,
    model: &str,
) -> bool {
    let translated = provider.model_map.get(model).map(String::as_str);
    provider.credential_pool.iter().any(|credential| {
        credential.enabled
            && !credential.discovery_required
            && credential_supports_model(credential, Some(model), translated)
            && provider.credential_pool_auth_policy.allows(credential)
    })
}

mod materialization;
pub(super) use materialization::apply_token_override;
use materialization::provider_to_candidate;
