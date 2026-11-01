//! OAuth management boundary: same operator/IP/host and secret grant rules as native pools.
use super::{chatgpt_oauth::authorize, json_with_no_store};
use crate::{
    console::antigravity_oauth, error::GatewayError, http::extractors::OptionalBearerToken,
    state::AppState,
};
use axum::{
    extract::{ConnectInfo, Path, State},
    http::HeaderMap,
    response::Response,
    Json,
};
use serde::Deserialize;
use std::{net::SocketAddr, sync::Arc};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Create {
    provider_id: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Action {
    #[serde(default)]
    callback_url: String,
}

pub async fn create_antigravity_oauth(
    State(state): State<Arc<AppState>>,
    info: Option<ConnectInfo<SocketAddr>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Json(body): Json<Create>,
) -> Result<Response, GatewayError> {
    let (owner, local) = authorize(&state, &headers, token.as_deref(), info.as_ref(), true)?;
    let session = antigravity_oauth::create(&state, owner, body.provider_id, local).await?;
    Ok(json_with_no_store(
        serde_json::json!({"session":session.view()}),
    ))
}
pub async fn get_antigravity_oauth(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    info: Option<ConnectInfo<SocketAddr>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
) -> Result<Response, GatewayError> {
    let (owner, _) = authorize(&state, &headers, token.as_deref(), info.as_ref(), false)?;
    Ok(json_with_no_store(
        serde_json::json!({"session":antigravity_oauth::get(&id,&owner)?.view()}),
    ))
}
pub async fn act_antigravity_oauth(
    State(state): State<Arc<AppState>>,
    Path((id, action)): Path<(String, String)>,
    info: Option<ConnectInfo<SocketAddr>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Json(body): Json<Action>,
) -> Result<Response, GatewayError> {
    let (owner, _) = authorize(
        &state,
        &headers,
        token.as_deref(),
        info.as_ref(),
        action == "import",
    )?;
    let session = antigravity_oauth::get(&id, &owner)?;
    match action.as_str() {
        "cancel" => session.cancel(),
        "complete" => {
            let (code, nonce) = antigravity_oauth::parse_callback(&body.callback_url)?;
            session
                .complete(state.upstream_client.client(), &code, &nonce)
                .await?;
        }
        "import" => {
            session.import(&state).await?;
        }
        _ => {
            return Err(GatewayError::bad_request(
                "Unsupported Antigravity OAuth action",
            ))
        }
    }
    Ok(json_with_no_store(
        serde_json::json!({"session":session.view()}),
    ))
}
