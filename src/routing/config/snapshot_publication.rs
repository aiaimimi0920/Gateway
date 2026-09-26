//! Validated snapshot assembly and reused/retired provider runtime state.

use super::*;

fn reuse_unchanged_providers(
    current: &RouteConfigSnapshot,
    replacement: &mut RouteConfigInner,
    replacement_fingerprints: &HashMap<String, String>,
) -> std::collections::HashSet<String> {
    let mut reused = std::collections::HashSet::new();
    let mut replacement_counts = HashMap::<String, usize>::new();
    for provider in &replacement.providers {
        *replacement_counts.entry(provider.id.clone()).or_default() += 1;
    }
    for provider in &mut replacement.providers {
        let id = provider.id.clone();
        let old_count = current
            .compiled()
            .providers
            .iter()
            .filter(|candidate| candidate.id == id)
            .count();
        let new_count = replacement_counts.get(&id).copied().unwrap_or_default();
        if old_count != 1 || new_count != 1 {
            continue;
        }
        let Some(old_fingerprint) = current.provider_fingerprints().get(&id) else {
            continue;
        };
        let Some(new_fingerprint) = replacement_fingerprints.get(&id) else {
            continue;
        };
        if old_fingerprint.is_empty()
            || old_fingerprint != new_fingerprint
            || new_fingerprint.is_empty()
        {
            continue;
        }
        let old_provider = current
            .compiled()
            .providers
            .iter()
            .find(|candidate| candidate.id == id)
            .expect("unique provider count checked above");
        *provider = old_provider.clone();
        reused.insert(id);
    }
    reused
}

fn retire_replaced_refresh_states(
    current: &RouteConfigSnapshot,
    reused_provider_ids: &std::collections::HashSet<String>,
) {
    for provider in &current.compiled().providers {
        if reused_provider_ids.contains(&provider.id) {
            continue;
        }
        for credential in &provider.credential_pool {
            if let Some(refresh) = &credential.refresh_config {
                refresh.retire();
            }
        }
    }
}

pub(super) fn validate_revision_matches_document(
    revision: &RevisionMetadata,
    validated: &ValidatedRouteDocument,
) -> Result<(), RouteConfigReplaceError> {
    revision
        .validate()
        .map_err(|error| RouteConfigReplaceError::InvalidRevision(error.to_string()))?;
    if revision.document_digest() != validated.document_digest()
        || revision.yaml_digest() != validated.yaml_digest()
    {
        return Err(RouteConfigReplaceError::RevisionDigestMismatch);
    }
    let canonical = canonicalize_route_document(validated.document())
        .map_err(|error| RouteConfigReplaceError::Compilation(error.to_string()))?;
    if canonical.canonical_json() != validated.canonical_json()
        || canonical.canonical_yaml() != validated.canonical_yaml()
        || canonical.document_digest() != validated.document_digest()
        || canonical.yaml_digest() != validated.yaml_digest()
    {
        return Err(RouteConfigReplaceError::ValidatedDocumentMismatch);
    }
    if validated.diagnostics().requires_repair() {
        return Err(RouteConfigReplaceError::Validation(
            validated.diagnostics().to_string(),
        ));
    }
    Ok(())
}

pub(super) fn build_snapshot(
    validated: ValidatedRouteDocument,
    revision: RevisionMetadata,
    source: ActiveConfigSource,
    current: Option<&Arc<RouteConfigSnapshot>>,
) -> Result<Arc<RouteConfigSnapshot>, RouteConfigReplaceError> {
    let (mut compiled, fingerprints) = compile_yaml_with_fingerprints(validated.document().clone())
        .map_err(|error| RouteConfigReplaceError::Compilation(error.to_string()))?;
    if let Some(current) = current {
        let reusable_ids = reuse_unchanged_providers(current, &mut compiled, &fingerprints);
        retire_replaced_refresh_states(current, &reusable_ids);
    }
    Ok(Arc::new(RouteConfigSnapshot::new(
        revision,
        source,
        validated.document().clone(),
        validated.diagnostics().clone(),
        compiled,
        fingerprints,
    )))
}
