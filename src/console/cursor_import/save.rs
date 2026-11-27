//! Reuse normal secret admission and atomic revision persistence; no second credential file or store.
use super::{error, Binding, Consent, Session};
use crate::{
    console::secrets::{redact_route_document, resolve_secret_patches, SecretPatch},
    error::GatewayError,
    protocol::cursor_auth::{self, Material},
    routing::config::{cursor_remote, RouteConfigSnapshot, RouteConfigYaml},
    state::AppState,
};
use serde_json::json;
use std::sync::Arc;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

pub(super) fn target(
    state: &AppState,
    binding: &Binding,
) -> Result<Arc<RouteConfigSnapshot>, GatewayError> {
    if state.local_runtime.is_none() {
        return Err(error("cursor_import_local_only"));
    }
    if state.shutdown.is_requested() || state.lifecycle.is_draining() {
        return Err(error("cursor_import_stopped"));
    }
    if state
        .route_config_runtime
        .as_ref()
        .is_none_or(|r| r.coordinator().is_none())
    {
        return Err(error("console_mutation_not_supported"));
    }
    let snapshot = state.route_config.snapshot();
    if snapshot.revision().id() != binding.revision {
        return Err(error("console_revision_conflict"));
    }
    let provider = snapshot
        .document()
        .providers
        .iter()
        .find(|p| p.id == binding.provider && cursor_remote::is_provider(p))
        .ok_or_else(|| error("cursor_import_target_invalid"))?;
    if binding.credential.as_ref().is_some_and(|id| {
        !provider
            .credentials
            .iter()
            .any(|c| c.id.as_ref() == Some(id))
    }) {
        return Err(error("cursor_import_target_invalid"));
    }
    Ok(snapshot)
}
fn prepare(
    state: &AppState,
    session: &Session,
    material: &Material,
) -> Result<(Arc<RouteConfigSnapshot>, RouteConfigYaml), GatewayError> {
    let snapshot = target(state, &session.binding)?;
    let mut redacted = redact_route_document(snapshot.document())
        .map_err(|_| error("cursor_import_save_rejected"))?;
    let pi = redacted
        .document
        .providers
        .iter()
        .position(|p| p.id == session.binding.provider)
        .unwrap();
    let original = &snapshot.document().providers[pi];
    let id = session.view().target_credential_id;
    // New authorization may update only its original account, never silently replace a different identity.
    for c in &original.credentials {
        if let Some(token) = &c.auth_token {
            let same_subject = cursor_auth::claims(token)?.sub == material.subject;
            if (c.id.as_ref() == Some(&id)) != same_subject {
                return Err(error("cursor_import_identity_conflict"));
            }
        }
    }
    let provider = &mut redacted.document.providers[pi];
    let expiry = OffsetDateTime::from_unix_timestamp(material.exp)
        .map_err(|_| error("cursor_invalid_token"))?
        .format(&Rfc3339)
        .map_err(|_| error("cursor_invalid_token"))?;
    let credential = serde_json::from_value(json!({
        "id":id, "account_name":material.email, "enabled":false, "expires_at":expiry,
    }))
    .map_err(|_| error("cursor_import_save_rejected"))?;
    let ci = if let Some(ci) = provider
        .credentials
        .iter()
        .position(|c| c.id.as_ref() == Some(&id))
    {
        provider.credentials[ci] = credential;
        ci
    } else {
        provider.credentials.push(credential);
        provider.credentials.len() - 1
    };
    let path = format!("/providers/{pi}/credentials/{ci}/auth_token");
    let mut patches: Vec<_> = redacted
        .secrets
        .into_iter()
        .filter(|d| d.path != path)
        .map(|d| SecretPatch::keep(d.path))
        .collect();
    patches.push(SecretPatch::replace(path, json!(material.token)));
    let document = resolve_secret_patches(snapshot.document(), redacted.document, &patches)
        .map_err(|_| error("cursor_import_save_rejected"))?;
    Ok((snapshot, document))
}
pub(super) async fn persist(
    state: &AppState,
    session: &Session,
    consent: &Consent,
    material: &Material,
) -> Result<(), GatewayError> {
    let mut stop = session.stop.subscribe();
    if *stop.borrow() {
        return Err(error("cursor_import_cancelled"));
    }
    let _publication = tokio::select! { biased;
        _ = stop.changed() => return Err(error("cursor_import_cancelled")),
        _ = tokio::time::sleep_until(session.deadline) => return Err(error("cursor_login_expired")),
        guard = state.lifecycle.health_control.publication.lock() => guard,
    };
    consent.check(state)?;
    cursor_auth::require_fresh(material.exp)?;
    let (active, document) = prepare(state, session, material)?;
    let unchanged = serde_json::to_value(&document)
        .map_err(|_| error("cursor_import_save_rejected"))?
        == serde_json::to_value(active.document())
            .map_err(|_| error("cursor_import_save_rejected"))?;
    {
        let mut view = session.inner.lock();
        if *session.stop.borrow() || tokio::time::Instant::now() >= session.deadline {
            return Err(error("cursor_login_expired"));
        }
        view.status = "importing";
    }
    // Request disconnect does not own this commit. An ambiguous outcome is read back, never auto-resubmitted.
    // A reauthorization can return the same token. Read back an exact no-op without opening a transaction.
    let result = if unchanged {
        active
    } else {
        state
            .route_config_runtime
            .as_ref()
            .unwrap()
            .commit_document(
                &session.binding.revision,
                document,
                Some("Import Cursor account; inference remains disabled".into()),
            )
            .await
            .map_err(|e| error(e.code()))?
    };
    let saved = result
        .document()
        .providers
        .iter()
        .find(|p| p.id == session.binding.provider)
        .and_then(|p| {
            p.credentials
                .iter()
                .find(|c| c.id.as_ref() == Some(&session.view().target_credential_id))
        })
        .filter(|c| c.auth_token.as_deref() == Some(&material.token));
    let Some(saved) = saved else {
        return Err(error("cursor_import_readback_failed"));
    };
    let mut view = session.inner.lock();
    view.status = "succeeded";
    view.credential_persisted = true;
    view.expires_at = saved.expires_at.clone();
    view.saved_revision = Some(result.revision().id().into());
    Ok(())
}

/// Generic configuration edits may preserve/remove records, but cannot impersonate verified import.
pub(crate) fn validate_generic_update(
    active: &RouteConfigYaml,
    candidate: &RouteConfigYaml,
) -> Result<(), GatewayError> {
    let protected: std::collections::HashSet<&str> = active
        .providers
        .iter()
        .filter(|p| cursor_remote::is_provider(p))
        .flat_map(|p| p.credentials.iter().filter_map(|c| c.auth_token.as_deref()))
        .collect();
    if !protected.is_empty() {
        // Keep is bound to IDs, not auth_mode. Never carry an import-only secret into a generic executor.
        for provider in candidate
            .providers
            .iter()
            .filter(|p| !cursor_remote::is_provider(p))
        {
            let value =
                serde_json::to_value(provider).map_err(|_| error("cursor_import_save_rejected"))?;
            if contains_protected_secret(&value, &protected) {
                return Err(error("cursor_import_required"));
            }
        }
    }
    for provider in candidate
        .providers
        .iter()
        .filter(|p| cursor_remote::is_provider(p))
    {
        let old = active
            .providers
            .iter()
            .find(|p| p.id == provider.id && cursor_remote::is_provider(p));
        for credential in provider
            .credentials
            .iter()
            .filter(|c| c.auth_token.is_some())
        {
            let unchanged = old
                .and_then(|p| p.credentials.iter().find(|c| c.id == credential.id))
                .is_some_and(|c| {
                    c.auth_token == credential.auth_token
                        && c.account_name == credential.account_name
                        && c.expires_at == credential.expires_at
                });
            if !unchanged {
                return Err(error("cursor_import_required"));
            }
        }
    }
    Ok(())
}

fn contains_protected_secret(
    value: &serde_json::Value,
    protected: &std::collections::HashSet<&str>,
) -> bool {
    match value {
        serde_json::Value::String(s) => protected.contains(s.as_str()),
        serde_json::Value::Array(items) => items
            .iter()
            .any(|v| contains_protected_secret(v, protected)),
        serde_json::Value::Object(items) => items
            .values()
            .any(|v| contains_protected_secret(v, protected)),
        _ => false,
    }
}
