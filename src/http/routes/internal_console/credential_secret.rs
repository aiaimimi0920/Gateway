//! Reveal only one account's effective API key behind the console secret grant.
use super::{console_request_context, json_with_no_store, required_console_management_token};
use crate::{error::GatewayError, http::extractors::OptionalBearerToken, state::AppState};
use axum::{
    extract::{ConnectInfo, State},
    http::HeaderMap,
    response::Response,
    Json,
};
use serde::Deserialize;
use std::{net::SocketAddr, sync::Arc};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CredentialSecretRequest {
    pub provider_id: String,
    pub credential_id: String,
    pub expected_revision: String,
}

pub async fn reveal_credential_key(
    State(state): State<Arc<AppState>>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Json(body): Json<CredentialSecretRequest>,
) -> Result<Response, GatewayError> {
    let request = console_request_context(&headers, connect_info.as_ref());
    let actor = state.console_auth.authenticate_management_token(
        &request,
        required_console_management_token(token.as_deref(), &headers)?,
    )?;
    let grant = headers
        .get("x-secret-grant")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    state
        .console_auth
        .verify_secret_grant(&request, &actor, grant)?;
    let snapshot = state.route_config.snapshot();
    if snapshot.revision().id() != body.expected_revision {
        return Err(GatewayError::conflict(
            "Configuration changed; reopen the account editor.",
        ));
    }
    let provider = snapshot
        .document()
        .providers
        .iter()
        .find(|p| p.id == body.provider_id)
        .ok_or_else(|| GatewayError::not_found("Account not found."))?;
    if !provider
        .credentials
        .iter()
        .any(|c| c.id.as_deref() == Some(&body.credential_id))
    {
        return Err(GatewayError::not_found("Account not found."));
    }
    let target = snapshot
        .select_credential_probe_target(&body.credential_id)
        .filter(|t| t.provider_id == body.provider_id)
        .ok_or_else(|| GatewayError::not_found("Account not found."))?;
    Ok(json_with_no_store(
        serde_json::json!({"apiKey": target.payload.api_key}),
    ))
}
