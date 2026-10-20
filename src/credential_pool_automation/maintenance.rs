//! Human-reviewed pruning uses existing driver, admission, archive and route-CAS owners.
//! A digest is an optimistic comparison, not an authorization token or stored ticket.
use super::{
    admission, archive, capacity, driver, inventory, DriverCredentialContext,
    DriverProviderContext, DriverRequest, PermanentPruneClassification,
};
use crate::{error::GatewayError, routing::config::RouteConfigSnapshot, state::AppState};
use inventory::effective_credential_id;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;
use uuid::Uuid;

mod apply;

const MAX_INSPECTED: usize = 1024;
const MAX_SELECTED: usize = 128;

// Blocking preservation owns the same admission until its actual I/O has ended,
// even when its awaiting HTTP request is cancelled or the archive budget expires.
struct MaintenanceAdmission {
    _process: tokio::sync::OwnedMutexGuard<()>,
    _shared: admission::ProviderAdmissionGuard,
}

async fn acquire(
    state: &AppState,
    provider_id: &str,
) -> Result<Arc<MaintenanceAdmission>, GatewayError> {
    if !state
        .route_config
        .snapshot()
        .document()
        .providers
        .iter()
        .any(|p| p.id == provider_id)
    {
        return Err(GatewayError::not_found(
            "Credential pool provider was not found",
        ));
    }
    let process = state
        .credential_pool_automation
        .lock_for_provider(provider_id)
        .try_lock_owned()
        .map_err(|_| busy())?;
    let shared = admission::acquire(state, provider_id).await?;
    Ok(Arc::new(MaintenanceAdmission {
        _process: process,
        _shared: shared,
    }))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PrunePreview {
    pub provider_id: String,
    pub revision: String,
    pub digest: String,
    pub archive_required: bool,
    pub items: Vec<PruneItem>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PruneItem {
    pub credential_id: String,
    pub classification: &'static str,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct PruneSelection {
    pub expected_revision: String,
    pub preview_digest: String,
    pub credential_ids: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PruneReceipt {
    pub provider_id: String,
    pub source_revision: String,
    pub revision: String,
    pub outcome: &'static str,
    pub removed_count: Option<usize>,
    pub items: Vec<PruneResult>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PruneResult {
    pub credential_id: String,
    pub status: &'static str,
}

pub(crate) async fn preview(
    state: &Arc<AppState>,
    provider_id: &str,
) -> Result<PrunePreview, GatewayError> {
    let _lease = acquire(state, provider_id).await?;
    let snapshot = state.route_config.snapshot();
    inspect(state, &snapshot, provider_id).await
}

pub(crate) async fn apply(
    state: &Arc<AppState>,
    provider_id: &str,
    selection: PruneSelection,
) -> Result<PruneReceipt, GatewayError> {
    validate_selection(&selection)?;
    let lease = acquire(state, provider_id).await?;
    let snapshot = state.route_config.snapshot();
    if snapshot.revision().id() != selection.expected_revision {
        return Err(stale());
    }
    // Re-run trusted classification, not a client-supplied list of failure strings.
    let current = inspect(state, &snapshot, provider_id).await?;
    if current.digest != selection.preview_digest
        || selection
            .credential_ids
            .iter()
            .any(|id| !current.items.iter().any(|row| &row.credential_id == id))
    {
        return Err(stale());
    }
    apply::commit_selection(state, &snapshot, provider_id, selection, lease).await
}

async fn inspect(
    state: &Arc<AppState>,
    snapshot: &RouteConfigSnapshot,
    provider_id: &str,
) -> Result<PrunePreview, GatewayError> {
    let runtime = &state.credential_pool_automation;
    if !runtime.enabled() {
        return Err(GatewayError::bad_request(
            "Credential pool automation is disabled",
        ));
    }
    let provider = snapshot
        .document()
        .providers
        .iter()
        .find(|p| p.id == provider_id)
        .ok_or_else(|| GatewayError::not_found("Credential pool provider was not found"))?;
    if provider.credentials.len() > MAX_INSPECTED {
        return Err(invalid("Inspect at most 1024 credentials in one pool"));
    }
    let driver = runtime
        .registry
        .resolve(provider)
        .ok()
        .flatten()
        .ok_or_else(|| invalid("A trusted credential driver is required"))?;
    let request = DriverRequest {
        run_id: Uuid::new_v4().to_string(),
        action: "prune",
        provider: DriverProviderContext {
            id: provider.id.clone(),
            label: provider
                .label
                .clone()
                .unwrap_or_else(|| provider.id.clone()),
            target_size: capacity::pool_max_size(provider),
            credential_count: provider.credentials.len(),
            active_credential_count: inventory::active_credential_count(provider),
            requested_count: 0,
            auto_refill_enabled: false,
            auto_prune_enabled: true,
            identity_categories: provider.credential_identity_categories.clone(),
            credentials: provider
                .credentials
                .iter()
                .enumerate()
                .map(|(index, credential)| DriverCredentialContext {
                    id: effective_credential_id(provider_id, index, credential),
                    account_name: credential.account_name.clone(),
                    enabled: credential.enabled.unwrap_or(true),
                    identity_category_id: credential.credential_identity_category_id.clone(),
                })
                .collect(),
        },
        refill: None,
    };
    let response = driver::execute_driver(&runtime.config, driver, &request)
        .await
        .map_err(|_| invalid("Credential inspection failed; no credentials were removed"))?;
    if state.route_config.snapshot().revision().id() != snapshot.revision().id() {
        return Err(stale());
    }
    if !response.credentials.is_empty() || response.prune.len() > MAX_INSPECTED {
        return Err(invalid("Invalid prune inspection response"));
    }
    let known: HashSet<_> = request
        .provider
        .credentials
        .iter()
        .map(|c| c.id.as_str())
        .collect();
    let mut decisions = BTreeMap::new();
    for decision in response.prune {
        if !known.contains(decision.credential_id.as_str()) {
            return Err(invalid(
                "Inspection referenced a credential outside this pool",
            ));
        }
        let class = match decision.classification {
            PermanentPruneClassification::PermanentAuthFailure => "permanent_auth_failure",
            PermanentPruneClassification::AccountDeleted => "account_deleted",
            PermanentPruneClassification::PermanentUpstreamRejection => {
                "permanent_upstream_rejection"
            }
        };
        if decisions
            .insert(decision.credential_id, class)
            .is_some_and(|old| old != class)
        {
            return Err(invalid("Inspection returned conflicting classifications"));
        }
    }
    let items: Vec<_> = decisions
        .into_iter()
        .map(|(credential_id, classification)| PruneItem {
            credential_id,
            classification,
        })
        .collect();
    let bytes = serde_json::to_vec(&(1, provider_id, snapshot.revision().id(), &items))
        .map_err(|_| GatewayError::server_error("Cannot encode credential preview"))?;
    Ok(PrunePreview {
        provider_id: provider_id.into(),
        revision: snapshot.revision().id().into(),
        digest: format!("{:x}", Sha256::digest(bytes)),
        archive_required: true,
        items,
    })
}

fn validate_selection(input: &PruneSelection) -> Result<(), GatewayError> {
    let unique: HashSet<_> = input.credential_ids.iter().collect();
    if input.expected_revision.is_empty()
        || input.expected_revision.len() > 256
        || input.preview_digest.len() != 64
        || !input.preview_digest.bytes().all(|b| b.is_ascii_hexdigit())
        || input.credential_ids.is_empty()
        || input.credential_ids.len() > MAX_SELECTED
        || unique.len() != input.credential_ids.len()
        || input.credential_ids.iter().any(|id| {
            id.is_empty() || id.len() > 256 || id.trim() != id || id.chars().any(char::is_control)
        })
    {
        return Err(invalid(
            "Select 1-128 unique credentials from a current preview",
        ));
    }
    Ok(())
}

fn invalid(message: &str) -> GatewayError {
    GatewayError::bad_request(message).with_code("credential_prune_inspection_failed")
}
fn stale() -> GatewayError {
    GatewayError::conflict(
        "Credential inspection changed; refresh the preview before selecting again",
    )
    .with_code("credential_prune_preview_stale")
}
fn busy() -> GatewayError {
    GatewayError::conflict("A credential pool operation is already running")
        .with_code("credential_pool_admission_busy")
}
