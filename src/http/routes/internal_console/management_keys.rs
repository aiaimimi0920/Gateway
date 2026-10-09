//! Administrator key management uses the same authenticated, no-store console boundary.
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
#[serde(tag = "action", rename_all = "camelCase", deny_unknown_fields)]
pub enum KeyMutation {
    Add {
        name: String,
        token: String,
    },
    Revoke {
        id: String,
    },
    Edit {
        id: String,
        name: String,
        token: Option<String>,
    },
    Reveal {
        id: String,
    },
}

pub async fn list_management_keys(
    State(state): State<Arc<AppState>>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
) -> Result<Response, GatewayError> {
    let request = console_request_context(&headers, connect_info.as_ref());
    let token = required_console_management_token(token.as_deref(), &headers)?.to_owned();
    let keys = tokio::task::spawn_blocking(move || {
        state.console_auth.list_management_keys(&request, &token)
    })
    .await
    .map_err(|_| GatewayError::service_unavailable("Management key task failed"))??;
    Ok(json_with_no_store(serde_json::json!({ "keys": keys })))
}

pub async fn mutate_management_key(
    State(state): State<Arc<AppState>>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Json(body): Json<KeyMutation>,
) -> Result<Response, GatewayError> {
    let request = console_request_context(&headers, connect_info.as_ref());
    let token = required_console_management_token(token.as_deref(), &headers)?.to_owned();
    let response = tokio::task::spawn_blocking(move || {
        match body {
            KeyMutation::Add {
                name,
                token: new_token,
            } => state
                .console_auth
                .add_management_key(&request, &token, &name, &new_token)?,
            KeyMutation::Revoke { id } => state
                .console_auth
                .revoke_management_key(&request, &token, &id)?,
            KeyMutation::Edit {
                id,
                name,
                token: replacement,
            } => state.console_auth.edit_management_key(
                &request,
                &token,
                &id,
                &name,
                replacement.as_deref(),
            )?,
            KeyMutation::Reveal { id } => {
                return state
                    .console_auth
                    .reveal_management_key(&request, &token, &id)
                    .map(|value| serde_json::json!({ "token": value }))
            }
        }
        Ok(serde_json::json!({ "success": true }))
    })
    .await
    .map_err(|_| GatewayError::service_unavailable("Management key task failed"))??;
    Ok(json_with_no_store(response))
}
