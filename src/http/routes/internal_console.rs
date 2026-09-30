//! Console wire types, shared request/response boundaries and Gemini session route wiring.

mod chatgpt_oauth;
mod probes;
pub use chatgpt_oauth::{act_chatgpt_oauth, create_chatgpt_oauth, get_chatgpt_oauth};
mod revisions;
mod route_config;
mod sessions;

pub use probes::{probe_console_credential, probe_console_provider};
pub use revisions::{get_route_config_revision, list_route_config_revisions};
pub use route_config::{commit_route_config, get_route_config, validate_route_config};
pub use sessions::{
    bootstrap_console_admin, confirm_console_secret_access, get_bootstrap_status,
    logout_console_session, rotate_console_session, verify_console_session,
};

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

use axum::extract::{ConnectInfo, Path, State};
use axum::http::{
    header::AUTHORIZATION,
    header::{CACHE_CONTROL, ETAG},
    HeaderMap, HeaderValue,
};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::console::secrets::{redact_route_document, SecretPatch};
use crate::console::{
    gemini_auth_session_manager, ConsoleRequestContext, CreateGeminiAuthSessionInput,
    GeminiAuthFamily, RouteConfigRuntimeError,
};
use crate::error::GatewayError;
use crate::http::extractors::OptionalBearerToken;
use crate::provider_runtime::ProviderPayloadProbeStatus;
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateGeminiAuthSessionRequest {
    pub target_family: GeminiAuthFamily,
    pub provider_id: String,
    #[serde(default)]
    pub account_label: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialProbeResponse {
    pub credential_id: String,
    pub provider_id: String,
    pub probe_point: String,
    pub status: ProviderPayloadProbeStatus,
    pub message: String,
    pub checked_at: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderProbeResponse {
    pub provider_id: String,
    pub status: ProviderPayloadProbeStatus,
    pub message: String,
    pub checked_at: String,
    pub total_count: usize,
    pub passed_count: usize,
    pub failed_count: usize,
    pub unsupported_count: usize,
    pub results: Vec<CredentialProbeResponse>,
}

pub async fn create_gemini_auth_session(
    State(state): State<Arc<AppState>>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Json(body): Json<CreateGeminiAuthSessionRequest>,
) -> Result<Response, GatewayError> {
    let request = console_request_context(&headers, connect_info.as_ref());
    state.console_auth.authenticate_management_token(
        &request,
        required_console_management_token(token.as_deref(), &headers)?,
    )?;
    let provider_id = body.provider_id.trim();
    if provider_id.is_empty() {
        return Err(
            GatewayError::bad_request("Gemini auth session providerId is required.")
                .with_code("console_gemini_auth_provider_required"),
        );
    }
    let session = gemini_auth_session_manager().create_session(CreateGeminiAuthSessionInput {
        target_family: body.target_family,
        provider_id: provider_id.to_string(),
        account_label: body.account_label,
    });
    Ok(json_with_no_store(
        serde_json::json!({ "session": session }),
    ))
}

pub async fn get_gemini_auth_session(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
) -> Result<Response, GatewayError> {
    let request = console_request_context(&headers, connect_info.as_ref());
    state.console_auth.authenticate_management_token(
        &request,
        required_console_management_token(token.as_deref(), &headers)?,
    )?;
    let normalized_session_id = session_id.trim();
    let session = gemini_auth_session_manager()
        .get_session(normalized_session_id)
        .ok_or_else(|| {
            GatewayError::not_found(format!(
                "Gateway Gemini auth session '{}' was not found",
                normalized_session_id
            ))
            .with_code("console_gemini_auth_session_not_found")
        })?;
    Ok(json_with_no_store(
        serde_json::json!({ "session": session }),
    ))
}

pub async fn complete_gemini_auth_session(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
) -> Result<Response, GatewayError> {
    let request = console_request_context(&headers, connect_info.as_ref());
    state.console_auth.authenticate_management_token(
        &request,
        required_console_management_token(token.as_deref(), &headers)?,
    )?;
    let normalized_session_id = session_id.trim();
    let manager = gemini_auth_session_manager();
    if manager.get_session(normalized_session_id).is_none() {
        return Err(GatewayError::not_found(format!(
            "Gateway Gemini auth session '{}' was not found",
            normalized_session_id
        ))
        .with_code("console_gemini_auth_session_not_found"));
    }
    let session = manager
        .request_manual_completion(normalized_session_id)
        .map_err(|message| {
            GatewayError::bad_request(message)
                .with_code("console_gemini_auth_manual_completion_failed")
        })?;
    Ok(json_with_no_store(
        serde_json::json!({ "session": session }),
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

pub(super) fn required_console_management_token<'a>(
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

pub(super) fn console_request_context(
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

pub(super) fn json_with_no_store(value: Value) -> Response {
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
