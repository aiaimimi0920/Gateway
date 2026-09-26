//! Queries pinned to one immutable route snapshot, including probe inventory.

use super::*;

impl std::fmt::Debug for RouteConfigSnapshot {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RouteConfigSnapshot")
            .field("revision", &self.revision)
            .field("source", &self.source)
            .field("provider_count", &self.compiled.providers.len())
            .field("route_count", &self.compiled.model_routes.len())
            .field("diagnostics", &self.diagnostics)
            .finish()
    }
}

impl RouteConfigSnapshot {
    pub(super) fn new(
        revision: RevisionMetadata,
        source: ActiveConfigSource,
        document: RouteConfigYaml,
        diagnostics: RouteConfigDiagnostics,
        compiled: RouteConfigInner,
        provider_fingerprints: HashMap<String, String>,
    ) -> Self {
        Self {
            revision,
            source,
            document,
            diagnostics,
            compiled: Arc::new(compiled),
            provider_fingerprints,
            account_group_inventory_cache: OnceLock::new(),
        }
    }

    pub fn revision(&self) -> &RevisionMetadata {
        &self.revision
    }

    pub fn source(&self) -> ActiveConfigSource {
        self.source
    }

    pub fn diagnostics(&self) -> &RouteConfigDiagnostics {
        &self.diagnostics
    }

    #[allow(dead_code)] // Consumed by the transaction runtime added in Task 5.
    pub(crate) fn document(&self) -> &RouteConfigYaml {
        &self.document
    }

    pub(crate) fn compiled(&self) -> &RouteConfigInner {
        self.compiled.as_ref()
    }

    pub(crate) fn provider_fingerprints(&self) -> &HashMap<String, String> {
        &self.provider_fingerprints
    }

    pub fn resolve_alias(&self, model: Option<&str>) -> Option<String> {
        model.and_then(|model| resolve_alias_inner(self.compiled(), model))
    }

    pub fn resolve_candidates(&self, model: Option<&str>) -> Vec<RouteCandidate> {
        resolve_candidates_inner(self.compiled(), model)
    }

    pub fn resolve_candidates_for_account_group(
        &self,
        model: Option<&str>,
        account_group_id: Option<&str>,
    ) -> Result<Vec<RouteCandidate>, RouteAccountGroupSelectionError> {
        let constraint = self.account_group_constraint(account_group_id)?;
        Ok(resolve_candidates_inner_with_account_filter(
            self.compiled(),
            model,
            constraint.allowed_account_ids.as_ref(),
        ))
    }

    pub fn account_group_constraint(
        &self,
        account_group_id: Option<&str>,
    ) -> Result<RouteAccountGroupConstraint, RouteAccountGroupSelectionError> {
        build_account_group_constraint(self.document(), account_group_id)
    }

    pub fn list_models(&self) -> Vec<ModelInfo> {
        list_models_inner(self.compiled())
    }

    pub fn has_routes(&self) -> bool {
        !self.compiled.providers.is_empty()
    }

    pub fn provider_count(&self) -> usize {
        self.compiled.providers.len()
    }

    pub(crate) fn get_providers(&self) -> Vec<CompiledProvider> {
        self.compiled.providers.clone()
    }

    pub fn account_group_inventory(&self) -> RouteAccountGroupInventory {
        self.account_group_inventory_cache
            .get_or_init(|| build_route_account_group_inventory(self.document(), self.compiled()))
            .clone()
    }

    pub(crate) fn select_credential_probe_target(
        &self,
        credential_id: &str,
    ) -> Option<CredentialProbeTarget> {
        let credential_id = credential_id.trim();
        if credential_id.is_empty() {
            return None;
        }

        for provider in &self.compiled.providers {
            if provider.credential_pool.is_empty() {
                let default_id = provider_default_account_id(&provider.id);
                if credential_id == default_id {
                    return Some(CredentialProbeTarget {
                        credential_id: default_id,
                        provider_id: provider.id.clone(),
                        enabled: true,
                        payload: provider.payload.clone(),
                    });
                }
                continue;
            }

            if let Some(credential) = provider
                .credential_pool
                .iter()
                .find(|credential| credential.id.trim() == credential_id)
            {
                let normalized_id = credential.id.trim().to_string();
                let mut payload = apply_token_override(credential);
                payload.credential_id = Some(normalized_id.clone());
                return Some(CredentialProbeTarget {
                    credential_id: normalized_id,
                    provider_id: provider.id.clone(),
                    enabled: credential.enabled,
                    payload,
                });
            }
        }

        None
    }

    pub(crate) fn credential_probe_targets_for_provider(
        &self,
        provider_id: &str,
    ) -> Option<Vec<CredentialProbeTarget>> {
        let provider_id = provider_id.trim();
        let provider = self
            .compiled
            .providers
            .iter()
            .find(|provider| provider.id == provider_id)?;
        if provider.credential_pool.is_empty() {
            return Some(vec![CredentialProbeTarget {
                credential_id: provider_default_account_id(&provider.id),
                provider_id: provider.id.clone(),
                enabled: true,
                payload: provider.payload.clone(),
            }]);
        }

        Some(
            provider
                .credential_pool
                .iter()
                .map(|credential| {
                    let credential_id = credential.id.trim().to_string();
                    let mut payload = apply_token_override(credential);
                    payload.credential_id = Some(credential_id.clone());
                    CredentialProbeTarget {
                        credential_id,
                        provider_id: provider.id.clone(),
                        enabled: credential.enabled,
                        payload,
                    }
                })
                .collect(),
        )
    }

    pub(crate) fn scheduled_credential_probe_targets(&self) -> Vec<ScheduledCredentialProbeTarget> {
        let mut targets = Vec::new();
        for provider in &self.compiled.providers {
            if provider.credential_pool.is_empty() {
                if provider.scheduled_probe_enabled {
                    targets.push(ScheduledCredentialProbeTarget {
                        target: CredentialProbeTarget {
                            credential_id: provider_default_account_id(&provider.id),
                            provider_id: provider.id.clone(),
                            enabled: true,
                            payload: provider.payload.clone(),
                        },
                        interval_minutes: provider
                            .scheduled_probe_interval_minutes
                            .clamp(1, 10_080),
                    });
                }
                continue;
            }

            targets.extend(provider.credential_pool.iter().filter_map(|credential| {
                credential.scheduled_probe_enabled.then(|| {
                    let credential_id = credential.id.trim().to_string();
                    let mut payload = apply_token_override(credential);
                    payload.credential_id = Some(credential_id.clone());
                    ScheduledCredentialProbeTarget {
                        target: CredentialProbeTarget {
                            credential_id,
                            provider_id: provider.id.clone(),
                            enabled: credential.enabled,
                            payload,
                        },
                        interval_minutes: credential
                            .scheduled_probe_interval_minutes
                            .clamp(1, 10_080),
                    }
                })
            }));
        }
        targets
    }
}
