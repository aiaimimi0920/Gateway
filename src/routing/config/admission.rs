//! Read-only catalogue admission: no credential selection, secret cloning, randomness or RR mutation.
use super::*;
#[cfg(test)]
mod tests;

impl RouteConfigSnapshot {
    pub(crate) fn model_inventory_for_admission(&self) -> Vec<ModelInfo> {
        model_catalog::list_model_inventory_inner(self.compiled())
    }

    /// Authorization diagnostics deliberately ignore manual disable and transient health.
    /// An entitled but unavailable account is not an invalid caller credential.
    pub(crate) fn has_authorized_model(
        &self,
        requested: Option<&str>,
        constraint: &RouteAccountGroupConstraint,
    ) -> bool {
        let guard = self.compiled();
        let alias = requested.and_then(|model| resolve_alias_inner(guard, model));
        let model = alias.as_deref().or(requested);
        let allowed = constraint.allowed_account_ids.as_ref();
        guard.providers.iter().any(|p| {
            let translated = model.and_then(|m| p.model_map.get(m).map(String::as_str));
            if p.credential_pool.is_empty() {
                return allowed.is_none_or(|ids| ids.contains(&provider_default_account_id(&p.id)))
                    && model.is_none_or(|m| {
                        p.supported_models.is_empty()
                            || model_mapping::provider_supports_model(p, m)
                    });
            }
            p.credential_pool.iter().any(|c| {
                allowed.is_none_or(|ids| ids.contains(&c.id))
                    && p.credential_pool_auth_policy.allows(c)
                    && (candidates::credential_supports_model(c, model, translated)
                        || model
                            .and_then(|m| p.model_map_targets.get(m))
                            .is_some_and(|targets| {
                                targets.iter().any(|target| {
                                    candidates::credential_supports_model(c, model, Some(target))
                                })
                            }))
            })
        })
    }

    pub(crate) fn has_admitted_candidate(
        &self,
        requested: &str,
        constraint: Option<&RouteAccountGroupConstraint>,
        providers: Option<&[String]>,
        admit: &dyn Fn(&str, &str, &str) -> bool,
    ) -> bool {
        let guard = self.compiled();
        let alias = resolve_alias_inner(guard, requested);
        let model = alias.as_deref().unwrap_or(requested);
        let accounts = constraint.and_then(|c| c.allowed_account_ids.as_ref());
        let available: HashSet<&str> = guard
            .providers
            .iter()
            .filter(|p| {
                providers.is_none_or(|ids| ids.contains(&p.id))
                    && provider_available(p, model, accounts, Some(admit))
            })
            .map(|p| p.id.as_str())
            .collect();
        model_catalog::model_has_available_candidate(guard, requested, &available)
            || (accounts.is_some()
                && guard.model_routes.iter().any(|r| {
                    r.enabled && glob_match(&r.pattern, model) && !r.provider_ids.is_empty()
                })
                && guard.providers.iter().any(|p| {
                    available.contains(p.id.as_str())
                        && model_mapping::provider_supports_model(p, model)
                }))
    }
}

fn provider_available(
    p: &CompiledProvider,
    model: &str,
    accounts: Option<&HashSet<String>>,
    admit: candidates::Admission<'_>,
) -> bool {
    if let Some(targets) = p.model_map_targets.get(model) {
        return targets.iter().any(|target| {
            model_mapping::eligible_credentials(p, model, target, accounts, admit).is_some()
        });
    }
    let upstream = p.model_map.get(model).map(String::as_str).unwrap_or(model);
    if p.credential_pool.is_empty() {
        let id = provider_default_account_id(&p.id);
        return p.credential_pool_auth_policy.is_any()
            && accounts.is_none_or(|ids| ids.contains(&id))
            && (accounts.is_none()
                || p.supported_models.is_empty()
                || model_mapping::provider_supports_model(p, model))
            && candidates::admits(admit, &p.id, &id, Some(upstream));
    }
    let available = |c: &&CompiledCredential| {
        c.enabled && !c.discovery_required && accounts.is_none_or(|ids| ids.contains(&c.id))
    };
    let matched = |c: &&CompiledCredential| {
        candidates::credential_supports_model(c, Some(model), Some(upstream))
            && p.credential_pool_auth_policy.allows(c)
    };
    let has_static_match = p
        .credential_pool
        .iter()
        .filter(available)
        .any(|c| matched(&c));
    p.credential_pool.iter().filter(available).any(|c| {
        (if has_static_match {
            matched(&c)
        } else {
            accounts.is_none() && p.credential_pool_auth_policy.is_any() && c.discovery.is_none()
        }) && candidates::admits(admit, &p.id, &c.id, Some(upstream))
    })
}
