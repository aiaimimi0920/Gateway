use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

use axum::extract::{ConnectInfo, Path, State};
use axum::http::{
    header::AUTHORIZATION,
    header::{CACHE_CONTROL, ETAG, IF_MATCH},
    HeaderMap, HeaderValue,
};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use serde_json::Value;

use crate::console::document::validate_route_document;
use crate::console::secrets::{redact_route_document, resolve_secret_patches, SecretPatch};
use crate::console::{
    ConsoleRequestContext, RouteConfigCoordinator, RouteConfigRuntimeError, StoredRouteRevision,
};
use crate::error::GatewayError;
use crate::http::extractors::OptionalBearerToken;
use crate::routing::config::{ActiveConfigSource, RouteConfigSnapshot, RouteConfigYaml};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BootstrapRequest {
    pub token: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfirmSecretAccessRequest {
    pub token: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RotateSessionRequest {
    pub new_token: String,
}

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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidateRouteConfigRequest {
    pub document: RouteConfigYaml,
    #[serde(default)]
    pub secret_patches: Vec<SecretPatch>,
}

pub async fn get_bootstrap_status(
    State(state): State<Arc<AppState>>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    headers: HeaderMap,
) -> Result<Response, GatewayError> {
    let request = console_request_context(&headers, connect_info.as_ref());
    let status = state.console_auth.bootstrap_status(&request)?;
    Ok(json_with_no_store(
        serde_json::to_value(status).expect("bootstrap status JSON"),
    ))
}

pub async fn bootstrap_console_admin(
    State(state): State<Arc<AppState>>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    headers: HeaderMap,
    Json(body): Json<BootstrapRequest>,
) -> Result<Response, GatewayError> {
    let request = console_request_context(&headers, connect_info.as_ref());
    state.console_auth.bootstrap(&request, &body.token)?;
    Ok(json_with_no_store(serde_json::json!({
        "success": true,
        "message": "Gateway console administrator configured"
    })))
}

pub async fn verify_console_session(
    State(state): State<Arc<AppState>>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
) -> Result<Response, GatewayError> {
    let request = console_request_context(&headers, connect_info.as_ref());
    let session = state.console_auth.verify_session(
        &request,
        required_console_management_token(token.as_deref(), &headers)?,
        Some(state.route_config.snapshot().revision().id()),
    )?;
    Ok(json_with_no_store(
        serde_json::to_value(session).expect("console session JSON"),
    ))
}

pub async fn confirm_console_secret_access(
    State(state): State<Arc<AppState>>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Json(body): Json<ConfirmSecretAccessRequest>,
) -> Result<Response, GatewayError> {
    let request = console_request_context(&headers, connect_info.as_ref());
    let grant = state.console_auth.confirm_secret_access(
        &request,
        required_console_management_token(token.as_deref(), &headers)?,
        &body.token,
    )?;
    Ok(json_with_no_store(
        serde_json::to_value(grant).expect("secret grant JSON"),
    ))
}

pub async fn rotate_console_session(
    State(state): State<Arc<AppState>>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Json(body): Json<RotateSessionRequest>,
) -> Result<Response, GatewayError> {
    let request = console_request_context(&headers, connect_info.as_ref());
    state.console_auth.rotate_token(
        &request,
        required_console_management_token(token.as_deref(), &headers)?,
        &body.new_token,
    )?;
    Ok(json_with_no_store(serde_json::json!({
        "success": true,
        "message": "Gateway console management token rotated"
    })))
}

pub async fn logout_console_session(
    State(state): State<Arc<AppState>>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
) -> Result<Response, GatewayError> {
    let request = console_request_context(&headers, connect_info.as_ref());
    state.console_auth.logout(
        &request,
        required_console_management_token(token.as_deref(), &headers)?,
    )?;
    Ok(json_with_no_store(serde_json::json!({
        "success": true,
        "message": "Gateway console session cleared"
    })))
}

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
    state.console_auth.authenticate_management_token(
        &request,
        required_console_management_token(token.as_deref(), &headers)?,
    )?;
    assert_if_match_consistent(&headers, &body.expected_revision)?;
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
    state.console_auth.authenticate_management_token(
        &request,
        required_console_management_token(token.as_deref(), &headers)?,
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

fn required_console_management_token<'a>(
    bearer_token: Option<&'a str>,
    headers: &'a HeaderMap,
) -> Result<&'a str, GatewayError> {
    headers
        .get("x-management-token")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .or_else(|| {
            headers
                .get("x-internal-api-key")
                .and_then(|value| value.to_str().ok())
                .map(str::trim)
                .filter(|value| !value.is_empty())
        })
        .or(bearer_token)
        .or_else(|| {
            headers
                .get(AUTHORIZATION)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.strip_prefix("Bearer "))
                .map(str::trim)
                .filter(|value| !value.is_empty())
        })
        .ok_or_else(|| {
            GatewayError::unauthorized("Management token is required")
                .with_code("console_management_token_invalid")
        })
}

fn console_request_context(
    headers: &HeaderMap,
    connect_info: Option<&ConnectInfo<SocketAddr>>,
) -> ConsoleRequestContext {
    let client_ip = connect_info
        .map(|connect| connect.0.ip())
        .or_else(|| {
            headers
                .get("x-gateway-client-ip")
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.trim().parse::<IpAddr>().ok())
        })
        .unwrap_or(IpAddr::from([127, 0, 0, 1]));
    let origin_key = headers
        .get("origin")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| {
            headers
                .get("host")
                .and_then(|value| value.to_str().ok())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
        })
        .unwrap_or_else(|| "local".to_string());
    ConsoleRequestContext::new(client_ip, origin_key)
}

fn json_with_no_store(value: Value) -> Response {
    let mut response = Json(value).into_response();
    response
        .headers_mut()
        .insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

fn json_with_no_store_and_etag(value: Value, revision_id: &str) -> Response {
    let mut response = json_with_no_store(value);
    response.headers_mut().insert(
        ETAG,
        HeaderValue::from_str(&format!("\"{revision_id}\"")).expect("valid revision etag"),
    );
    response
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

fn active_source_name(source: ActiveConfigSource) -> &'static str {
    match source {
        ActiveConfigSource::Yaml => "yaml",
        ActiveConfigSource::Redis => "redis",
        ActiveConfigSource::Database => "database",
        ActiveConfigSource::Recovered => "recovered",
    }
}

fn secret_patch_to_gateway_error(error: crate::console::secrets::SecretPatchError) -> GatewayError {
    let mut gateway_error = GatewayError::bad_request(error.to_string()).with_code(error.code());
    if matches!(
        error.code(),
        "secret_mask_sentinel_rejected" | "secret_value_must_use_patch"
    ) {
        gateway_error.http_status = Some(422);
    }
    gateway_error
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
