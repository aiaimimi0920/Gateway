//! Public model discovery filtered by available runtime accounts.

use super::*;

pub(super) fn list_models_inner(guard: &RouteConfigInner) -> Vec<ModelInfo> {
    let mut model_ids: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();

    // Provider supported_models lists.
    for p in &guard.providers {
        for m in &p.supported_models {
            model_ids.insert(m.clone());
        }
    }

    // Exact-match route patterns (no wildcard).
    for r in &guard.model_routes {
        if !r.pattern.contains('*') {
            model_ids.insert(r.pattern.clone());
        }
    }

    // Alias keys are the platform-facing names users should request
    // directly. Do not also expose alias targets here because they may be
    // provider-native model identifiers.
    for alias in guard.aliases.keys() {
        model_ids.insert(alias.clone());
    }

    // Discovery must describe models that can produce at least one concrete
    // runtime candidate. A provider with a credential pool has no default
    // account fallback, so an all-disabled pool cannot keep its models, exact
    // routes, or aliases visible. Providers without `credentials` retain their
    // legacy default-account behavior.
    let available_provider_ids = guard
        .providers
        .iter()
        .filter(|provider| provider_has_available_account(provider))
        .map(|provider| provider.id.as_str())
        .collect::<HashSet<_>>();
    model_ids.retain(|model| model_has_available_candidate(guard, model, &available_provider_ids));

    let created = created_timestamp();

    model_ids
        .into_iter()
        .map(|id| ModelInfo {
            object: "model".to_string(),
            owned_by: "neuro-gateway".to_string(),
            created,
            id,
        })
        .collect()
}

fn provider_has_available_account(provider: &CompiledProvider) -> bool {
    provider.credential_pool.is_empty()
        || provider
            .credential_pool
            .iter()
            .any(|credential| credential.enabled)
}

fn model_has_available_candidate(
    guard: &RouteConfigInner,
    requested_model: &str,
    available_provider_ids: &HashSet<&str>,
) -> bool {
    let resolved_alias = resolve_alias_inner(guard, requested_model);
    let resolved_model = resolved_alias.as_deref().unwrap_or(requested_model);

    let mut matched_route = false;
    for route in guard
        .model_routes
        .iter()
        .filter(|route| glob_match(&route.pattern, resolved_model))
    {
        matched_route = true;
        // Mirrors `resolve`: a disabled rule matches but serves nobody, so the
        // model drops out of the listing instead of falling through.
        if !route.enabled {
            continue;
        }
        if route
            .provider_ids
            .iter()
            .any(|provider_id| available_provider_ids.contains(provider_id.as_str()))
        {
            return true;
        }
    }
    if matched_route {
        return false;
    }

    let mut explicitly_supported = false;
    for provider in &guard.providers {
        let upstream_model = provider.model_map.get(resolved_model).map(String::as_str);
        let supports_model = !provider.supported_models.is_empty()
            && provider.supported_models.iter().any(|supported| {
                supported == resolved_model || upstream_model == Some(supported.as_str())
            });
        if supports_model {
            explicitly_supported = true;
            if available_provider_ids.contains(provider.id.as_str()) {
                return true;
            }
        }
    }

    !explicitly_supported && !available_provider_ids.is_empty()
}

/// A fixed "created" timestamp (2026-01-01T00:00:00Z as UNIX seconds).
/// We use a constant rather than the current time so the response is stable.
fn created_timestamp() -> i64 {
    1_767_225_600 // 2026-01-01T00:00:00Z
}
