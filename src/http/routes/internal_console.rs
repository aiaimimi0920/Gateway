use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::Json;
use serde::Deserialize;
use serde_json::Value;

use crate::console::secrets::{redact_route_document, resolve_secret_patches, SecretPatch};
use crate::console::{RouteConfigCoordinator, RouteConfigRuntimeError, StoredRouteRevision};
use crate::error::GatewayError;
use crate::http::extractors::OptionalBearerToken;
use crate::routing::config::{ActiveConfigSource, RouteConfigSnapshot, RouteConfigYaml};
use crate::state::AppState;

use super::internal_gateway::assert_management_access;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitRouteConfigRequest {
    pub expected_revision: String,
    pub document: RouteConfigYaml,
    #[serde(default)]
    pub secret_patches: Vec<SecretPatch>,
    #[serde(default)]
    pub message: Option<String>,
}

pub async fn get_route_config(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    Ok(Json(serde_json::json!({
        "routeConfig": route_config_payload(state.as_ref(), &state.route_config.snapshot())?
    })))
}

pub async fn commit_route_config(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Json(body): Json<CommitRouteConfigRequest>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let runtime = state.route_config_runtime.as_ref().ok_or_else(|| {
        GatewayError::service_unavailable(
            "Gateway console runtime is not configured for route mutations",
        )
        .with_code("console_runtime_unavailable")
    })?;
    let active = state.route_config.snapshot();
    let candidate = if body.secret_patches.is_empty() {
        body.document
    } else {
        resolve_secret_patches(active.document(), body.document, &body.secret_patches)
            .map_err(secret_patch_to_gateway_error)?
    };
    let snapshot = runtime
        .commit_document(&body.expected_revision, candidate, body.message)
        .await
        .map_err(runtime_error_to_gateway_error)?;
    Ok(Json(serde_json::json!({
        "routeConfig": route_config_payload(state.as_ref(), &snapshot)?,
        "committed": true
    })))
}

pub async fn list_route_config_revisions(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let coordinator = route_config_coordinator(state.as_ref())?;
    let active = state.route_config.snapshot();
    let stored = coordinator
        .load_revisions()
        .map_err(runtime_error_to_gateway_error)?;
    let mut revisions: Vec<Value> = stored
        .iter()
        .map(|revision| revision_summary_payload(active.as_ref(), revision))
        .collect();
    if !stored
        .iter()
        .any(|revision| revision.metadata().id() == active.revision().id())
    {
        revisions.insert(0, current_revision_summary_payload(active.as_ref(), false));
    }
    Ok(Json(serde_json::json!({ "revisions": revisions })))
}

pub async fn get_route_config_revision(
    State(state): State<Arc<AppState>>,
    Path(revision_id): Path<String>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let coordinator = route_config_coordinator(state.as_ref())?;
    let active = state.route_config.snapshot();
    if active.revision().id() == revision_id {
        let has_archive = coordinator
            .load_revision(&revision_id)
            .map_err(runtime_error_to_gateway_error)?
            .is_some();
        return Ok(Json(serde_json::json!({
            "routeConfig": route_config_payload(state.as_ref(), &active)?,
            "active": true,
            "hasArchive": has_archive,
        })));
    }

    let stored = coordinator
        .load_revision(&revision_id)
        .map_err(runtime_error_to_gateway_error)?
        .ok_or_else(|| {
            GatewayError::not_found(format!(
                "Gateway console revision '{}' was not found",
                revision_id
            ))
            .with_code("console_revision_not_found")
        })?;
    Ok(Json(serde_json::json!({
        "routeConfig": archived_route_config_payload(state.as_ref(), &stored)?,
        "active": false,
        "hasArchive": true,
    })))
}

fn route_config_payload(
    state: &AppState,
    snapshot: &Arc<RouteConfigSnapshot>,
) -> Result<Value, GatewayError> {
    let redacted =
        redact_route_document(snapshot.document()).map_err(secret_patch_to_gateway_error)?;
    Ok(serde_json::json!({
        "revision": snapshot.revision(),
        "source": active_source_name(snapshot.source()),
        "diagnostics": snapshot.diagnostics(),
        "requiresRepair": snapshot.diagnostics().requires_repair(),
        "document": redacted.document,
        "secrets": redacted.secrets,
        "mutationSupported": state
            .route_config_runtime
            .as_ref()
            .and_then(|runtime| runtime.coordinator())
            .is_some(),
    }))
}

fn archived_route_config_payload(
    state: &AppState,
    stored: &StoredRouteRevision,
) -> Result<Value, GatewayError> {
    let redacted =
        redact_route_document(stored.document()).map_err(secret_patch_to_gateway_error)?;
    Ok(serde_json::json!({
        "revision": stored.metadata(),
        "source": "archived",
        "diagnostics": Value::Null,
        "requiresRepair": false,
        "document": redacted.document,
        "secrets": redacted.secrets,
        "mutationSupported": state
            .route_config_runtime
            .as_ref()
            .and_then(|runtime| runtime.coordinator())
            .is_some(),
    }))
}

fn current_revision_summary_payload(snapshot: &RouteConfigSnapshot, has_archive: bool) -> Value {
    serde_json::json!({
        "revision": snapshot.revision(),
        "active": true,
        "hasArchive": has_archive,
        "source": active_source_name(snapshot.source()),
    })
}

fn revision_summary_payload(active: &RouteConfigSnapshot, stored: &StoredRouteRevision) -> Value {
    serde_json::json!({
        "revision": stored.metadata(),
        "active": stored.metadata().id() == active.revision().id(),
        "hasArchive": true,
        "source": if stored.metadata().id() == active.revision().id() {
            active_source_name(active.source())
        } else {
            "archived"
        },
    })
}

fn route_config_coordinator(state: &AppState) -> Result<&RouteConfigCoordinator, GatewayError> {
    state
        .route_config_runtime
        .as_ref()
        .and_then(|runtime| runtime.coordinator())
        .ok_or_else(|| {
            GatewayError::service_unavailable(
                "Gateway console runtime is not configured for revision history",
            )
            .with_code("console_runtime_unavailable")
        })
}

fn active_source_name(source: ActiveConfigSource) -> &'static str {
    match source {
        ActiveConfigSource::Yaml => "yaml",
        ActiveConfigSource::Redis => "redis",
        ActiveConfigSource::Database => "database",
        ActiveConfigSource::Recovered => "recovered",
    }
}

fn secret_patch_to_gateway_error(error: crate::console::secrets::SecretPatchError) -> GatewayError {
    GatewayError::bad_request(error.to_string()).with_code(error.code())
}

fn runtime_error_to_gateway_error(error: RouteConfigRuntimeError) -> GatewayError {
    match error.code() {
        "console_revision_conflict" => {
            GatewayError::conflict(error.to_string()).with_code(error.code())
        }
        "console_route_validation_failed" | "console_mutation_not_supported" => {
            GatewayError::bad_request(error.to_string()).with_code(error.code())
        }
        "console_redis_unavailable"
        | "console_redis_invalid_state"
        | "console_recovery_required"
        | "console_redis_indeterminate" => {
            GatewayError::service_unavailable(error.to_string()).with_code(error.code())
        }
        code if code.starts_with("console_") => {
            GatewayError::bad_request(error.to_string()).with_code(code)
        }
        _ => GatewayError::service_unavailable(error.to_string()).with_code(error.code()),
    }
}
