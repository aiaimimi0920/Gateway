//! Secret-grant protected local unlimited-key import; no implicit create semantics or quota reset.
use super::access_keys::AccessKeyBody;
use crate::http::routes::internal_console::{
    console_request_context, json_with_no_store, required_console_management_token,
};
use crate::{error::GatewayError, http::extractors::OptionalBearerToken, state::AppState};
use axum::{
    extract::{ConnectInfo, State},
    http::HeaderMap,
    response::Response,
    Json,
};
use serde::Deserialize;
use std::{net::SocketAddr, sync::Arc};

// Deliberately no Debug: the request contains a serving secret.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImportKeyBody {
    token: String,
    key: AccessKeyBody,
}

pub async fn import_unlimited_access_key(
    State(state): State<Arc<AppState>>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Json(body): Json<ImportKeyBody>,
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
    let local = state
        .local_runtime
        .as_ref()
        .ok_or_else(|| GatewayError::bad_request("Key import requires local storage"))?;
    if body.key.quota.is_some() {
        return Err(GatewayError::bad_request(
            "This import accepts active unlimited keys only; quota migration is not supported",
        ));
    }
    let input = body.key.into();
    crate::access_key_groups::validate_input(&state, &input)?;
    let key = local
        .import_unlimited_access_key(input, &body.token)
        .await?;
    Ok(json_with_no_store(serde_json::json!(key)))
}
