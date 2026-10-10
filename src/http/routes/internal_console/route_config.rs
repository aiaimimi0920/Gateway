//! Route reads, strict validation and commits share revision and protected-secret admission.

use super::{
    console_request_context, json_with_no_store, json_with_no_store_and_etag,
    required_console_management_token, route_config_payload, runtime_error_to_gateway_error,
    secret_patch_to_gateway_error, CommitRouteConfigRequest, ValidateRouteConfigRequest,
};
use crate::console::document::validate_route_document;
use crate::console::secrets::{
    redact_route_document, resolve_secret_patches, route_config_request_requires_secret_grant,
    SecretPatch,
};
use crate::console::{AuthenticatedConsoleActor, ConsoleRequestContext};
use crate::error::GatewayError;
use crate::http::extractors::OptionalBearerToken;
use crate::routing::config::RouteConfigYaml;
use crate::state::AppState;
use axum::extract::{ConnectInfo, State};
use axum::http::{header::IF_MATCH, HeaderMap};
use axum::response::Response;
use axum::Json;
use std::net::SocketAddr;
use std::sync::Arc;

pub async fn get_route_config(
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
    let snapshot = state.route_config.snapshot();
    Ok(json_with_no_store_and_etag(
        serde_json::json!({
            "routeConfig": route_config_payload(state.as_ref(), &snapshot)?
        }),
        snapshot.revision().id(),
    ))
}

pub async fn commit_route_config(
    State(state): State<Arc<AppState>>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Json(body): Json<CommitRouteConfigRequest>,
) -> Result<Response, GatewayError> {
    let request = console_request_context(&headers, connect_info.as_ref());
    let actor = state.console_auth.authenticate_management_token(
        &request,
        required_console_management_token(token.as_deref(), &headers)?,
    )?;
    verify_route_config_secret_grant_if_required(
        state.as_ref(),
        &request,
        &actor,
        &headers,
        &body.document,
        &body.secret_patches,
    )?;
    assert_if_match_consistent(&headers, &body.expected_revision)?;
    let runtime = state.route_config_runtime.as_ref().ok_or_else(|| {
        GatewayError::service_unavailable(
            "Gateway console runtime is not configured for route mutations",
        )
        .with_code("console_runtime_unavailable")
    })?;
    let active = state.route_config.snapshot();
    let candidate = resolve_secret_patches(active.document(), body.document, &body.secret_patches)
        .map_err(secret_patch_to_gateway_error)?;
    crate::provider_discovery::job::validate(&candidate)
        .map_err(|error| GatewayError::bad_request(error.to_string()))?;
    if crate::provider_discovery::job::new_request(active.document(), &candidate) {
        let grant = headers
            .get("x-secret-grant")
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default();
        state
            .console_auth
            .verify_secret_grant(&request, &actor, grant)?;
    }
    let snapshot = runtime
        .commit_document(&body.expected_revision, candidate, body.message)
        .await
        .map_err(runtime_error_to_gateway_error)?;
    crate::provider_discovery::background::schedule(&state);
    Ok(json_with_no_store(serde_json::json!({
        "routeConfig": route_config_payload(state.as_ref(), &snapshot)?,
        "committed": true
    })))
}

pub async fn validate_route_config(
    State(state): State<Arc<AppState>>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Json(body): Json<ValidateRouteConfigRequest>,
) -> Result<Response, GatewayError> {
    let request = console_request_context(&headers, connect_info.as_ref());
    let actor = state.console_auth.authenticate_management_token(
        &request,
        required_console_management_token(token.as_deref(), &headers)?,
    )?;
    verify_route_config_secret_grant_if_required(
        state.as_ref(),
        &request,
        &actor,
        &headers,
        &body.document,
        &body.secret_patches,
    )?;
    let active = state.route_config.snapshot();
    let candidate = resolve_secret_patches(active.document(), body.document, &body.secret_patches)
        .map_err(secret_patch_to_gateway_error)?;
    let validated = validate_route_document(candidate).map_err(|diagnostics| {
        let mut error = GatewayError::bad_request(diagnostics.to_string())
            .with_code("console_route_validation_failed");
        error.http_status = Some(422);
        error
    })?;
    let redacted =
        redact_route_document(validated.document()).map_err(secret_patch_to_gateway_error)?;
    Ok(json_with_no_store(serde_json::json!({
        "validation": {
            "document": redacted.document,
            "secrets": redacted.secrets,
            "diagnostics": validated.diagnostics(),
            "requiresRepair": validated.diagnostics().requires_repair(),
        }
    })))
}

fn verify_route_config_secret_grant_if_required(
    state: &AppState,
    request: &ConsoleRequestContext,
    actor: &AuthenticatedConsoleActor,
    headers: &HeaderMap,
    document: &RouteConfigYaml,
    secret_patches: &[SecretPatch],
) -> Result<(), GatewayError> {
    if !route_config_request_requires_secret_grant(document, secret_patches) {
        return Ok(());
    }
    let secret_grant = headers
        .get("x-secret-grant")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    state
        .console_auth
        .verify_secret_grant(request, actor, secret_grant)
}

fn assert_if_match_consistent(
    headers: &HeaderMap,
    expected_revision: &str,
) -> Result<(), GatewayError> {
    let Some(if_match) = headers.get(IF_MATCH).and_then(|value| value.to_str().ok()) else {
        return Ok(());
    };
    let if_match = if_match.trim().trim_matches('"');
    if if_match.is_empty() || if_match == expected_revision {
        return Ok(());
    }
    Err(GatewayError::bad_request(format!(
        "If-Match revision '{}' does not match expectedRevision '{}'",
        if_match, expected_revision
    ))
    .with_code("console_if_match_mismatch"))
}
