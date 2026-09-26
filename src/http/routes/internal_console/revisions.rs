//! Revision history preserves active/archive selection, redaction, ordering and ETags.

use super::{
    active_source_name, console_request_context, json_with_no_store, json_with_no_store_and_etag,
    required_console_management_token, route_config_payload, runtime_error_to_gateway_error,
    secret_patch_to_gateway_error,
};
use crate::console::secrets::redact_route_document;
use crate::console::{RouteConfigCoordinator, StoredRouteRevision};
use crate::error::GatewayError;
use crate::http::extractors::OptionalBearerToken;
use crate::routing::config::RouteConfigSnapshot;
use crate::state::AppState;
use axum::extract::{ConnectInfo, Path, State};
use axum::http::HeaderMap;
use axum::response::Response;
use serde_json::Value;
use std::net::SocketAddr;
use std::sync::Arc;

pub async fn list_route_config_revisions(
    State(state): State<Arc<AppState>>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
) -> Result<Response, GatewayError> {
    let request = console_request_context(&headers, connect_info.as_ref());
    state.console_auth.authenticate_management_token(
        &request,
        required_console_management_token(token.as_deref(), &headers)?,
    )?;
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
    Ok(json_with_no_store(
        serde_json::json!({ "revisions": revisions }),
    ))
}

pub async fn get_route_config_revision(
    State(state): State<Arc<AppState>>,
    Path(revision_id): Path<String>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
) -> Result<Response, GatewayError> {
    let request = console_request_context(&headers, connect_info.as_ref());
    state.console_auth.authenticate_management_token(
        &request,
        required_console_management_token(token.as_deref(), &headers)?,
    )?;
    let coordinator = route_config_coordinator(state.as_ref())?;
    let active = state.route_config.snapshot();
    if active.revision().id() == revision_id {
        let has_archive = coordinator
            .load_revision(&revision_id)
            .map_err(runtime_error_to_gateway_error)?
            .is_some();
        return Ok(json_with_no_store_and_etag(
            serde_json::json!({
                "routeConfig": route_config_payload(state.as_ref(), &active)?,
                "active": true,
                "hasArchive": has_archive,
            }),
            active.revision().id(),
        ));
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
    Ok(json_with_no_store_and_etag(
        serde_json::json!({
            "routeConfig": archived_route_config_payload(state.as_ref(), &stored)?,
            "active": false,
            "hasArchive": true,
        }),
        stored.metadata().id(),
    ))
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
