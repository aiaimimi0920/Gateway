//! Read-only account directory; normal form editing and route CAS own model publication.
use super::{chatgpt_oauth::authorize, json_with_no_store};
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
pub struct Input {
    provider_id: String,
    credential_id: String,
    expected_revision: String,
}
pub(super) async fn read(
    State(state): State<Arc<AppState>>,
    info: Option<ConnectInfo<SocketAddr>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Json(input): Json<Input>,
) -> Result<Response, GatewayError> {
    let (owner, _) = authorize(&state, &headers, token.as_deref(), info.as_ref(), true)?;
    let binding = crate::console::lobsterai_oauth::Binding::new(
        &owner,
        &input.provider_id,
        &input.expected_revision,
    )
    .and_then(|b| b.with_credential(&input.credential_id))
    .map_err(crate::console::lobsterai_oauth::login_error)?;
    let snapshot = crate::console::lobsterai_oauth::validate_target(&state, &binding)?;
    let credential = snapshot
        .document()
        .providers
        .iter()
        .find(|p| p.id == input.provider_id)
        .and_then(|p| {
            p.credentials
                .iter()
                .find(|c| c.id.as_ref() == Some(&input.credential_id))
        })
        .ok_or_else(|| GatewayError::bad_request("LobsterAI account not found"))?;
    let tokens = crate::console::lobsterai_oauth::stored(credential)?;
    let _slot = crate::provider_discovery::background::SLOTS
        .try_acquire()
        .map_err(|_| GatewayError::conflict("Discovery is busy; try again shortly."))?;
    let catalog = tokens.model_catalog().await.map_err(|_| {
        GatewayError::service_unavailable("LobsterAI model directory read failed.")
            .with_code("lobsterai_catalog_read_failed")
    })?;
    authorize(&state, &headers, token.as_deref(), info.as_ref(), true)?;
    crate::console::lobsterai_oauth::validate_target(&state, &binding)?;
    Ok(json_with_no_store(
        serde_json::json!({"revision":snapshot.revision().id(),
        "providerId":input.provider_id,"credentialId":input.credential_id,
        "catalog":{"models":catalog}}),
    ))
}
