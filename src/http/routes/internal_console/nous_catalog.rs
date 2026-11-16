//! Read-only account directory; normal form editing and route CAS own model publication.
use super::{chatgpt_oauth::authorize, json_with_no_store};
use crate::{
    error::GatewayError, http::extractors::OptionalBearerToken, protocol::nous_portal::business,
    state::AppState,
};
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
    let binding = crate::console::nous_oauth::Binding::new(
        &owner,
        &input.provider_id,
        &input.expected_revision,
    )
    .and_then(|b| b.with_credential(&input.credential_id))
    .map_err(crate::console::nous_oauth::login_error)?;
    let snapshot = crate::console::nous_oauth::validate_target(&state, &binding)?;
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
        .ok_or_else(business::invalid)?;
    let context = credential
        .nous_portal
        .as_ref()
        .ok_or_else(business::invalid)?;
    let access = credential
        .api_key
        .as_deref()
        .ok_or_else(business::invalid)?;
    let _slot = crate::provider_discovery::background::SLOTS
        .try_acquire()
        .map_err(|_| GatewayError::conflict("Discovery is busy; try again shortly."))?;
    let catalog = context
        .model_catalog(state.upstream_client.client(), access)
        .await
        .map_err(|_| {
            GatewayError::service_unavailable("Nous model directory read failed.")
                .with_code("nous_catalog_read_failed")
        })?;
    authorize(&state, &headers, token.as_deref(), info.as_ref(), true)?;
    crate::console::nous_oauth::validate_target(&state, &binding)?;
    Ok(json_with_no_store(
        serde_json::json!({"revision":snapshot.revision().id(),
        "providerId":input.provider_id,"credentialId":input.credential_id,
        "catalog":{"models":catalog.models}}),
    ))
}
