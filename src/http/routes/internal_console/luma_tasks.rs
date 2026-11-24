//! Management-only task read, bound to a saved account and unchanged configuration revision.
use super::{chatgpt_oauth::authorize, json_with_no_store};
use crate::{
    error::GatewayError, http::extractors::OptionalBearerToken, protocol::luma_agents::PRESET_ID,
    state::AppState, upstream::luma_agents::task_lookup,
};
use axum::{
    extract::{ConnectInfo, State},
    http::HeaderMap,
    response::Response,
    Json, Router,
};
use serde::Deserialize;
use std::{net::SocketAddr, sync::Arc};

pub(crate) fn mount(router: Router<Arc<AppState>>) -> Router<Arc<AppState>> {
    router.route(
        "/v1/internal/gateway/console/luma-agents-task",
        axum::routing::post(read)
            .layer(axum::extract::DefaultBodyLimit::max(4096))
            .layer(axum::middleware::map_response(no_store)),
    )
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Input {
    provider_id: String,
    credential_id: String,
    expected_revision: String,
    generation_id: String,
}
async fn read(
    State(state): State<Arc<AppState>>,
    info: Option<ConnectInfo<SocketAddr>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Json(input): Json<Input>,
) -> Result<Response, GatewayError> {
    authorize(&state, &headers, token.as_deref(), info.as_ref(), true)?;
    if state.shutdown.is_requested() || state.lifecycle.is_draining() {
        return Err(GatewayError::service_unavailable("Gateway is stopping."));
    }
    let _slot = crate::provider_discovery::background::SLOTS
        .try_acquire()
        .map_err(|_| GatewayError::conflict("Account operations are busy; try again shortly."))?;
    let id = task_lookup::task_id(&input.generation_id)?;
    let snapshot = state.route_config.snapshot();
    if snapshot.revision().id() != input.expected_revision {
        return Err(revision_changed());
    }
    let invalid = || {
        GatewayError::bad_request("Select a saved, enabled Luma Agents account.")
            .with_code("luma_task_account")
    };
    let provider = snapshot
        .document()
        .providers
        .iter()
        .find(|p| p.id == input.provider_id && p.preset.as_deref() == Some(PRESET_ID))
        .ok_or_else(invalid)?;
    if !provider
        .credentials
        .iter()
        .any(|c| c.id.as_deref() == Some(input.credential_id.as_str()) && c.enabled != Some(false))
    {
        return Err(invalid());
    }
    let target = snapshot
        .credential_probe_targets_for_provider(&input.provider_id)
        .ok_or_else(invalid)?
        .into_iter()
        .find(|t| t.credential_id == input.credential_id && t.enabled)
        .ok_or_else(invalid)?;
    #[cfg(not(test))]
    let http = rquest::Client::builder()
        .build()
        .map_err(|_| GatewayError::service_unavailable("Luma task transport unavailable."))?;
    #[cfg(test)]
    let http = state.upstream_client.http.clone();
    let result = task_lookup::execute(&http, &target.payload, &id).await?;
    // A revoked grant or edited account must not receive a late response from the old scope.
    authorize(&state, &headers, token.as_deref(), info.as_ref(), true)?;
    if state.route_config.snapshot().revision().id() != snapshot.revision().id() {
        return Err(revision_changed());
    }
    Ok(json_with_no_store(
        serde_json::json!({"revision":snapshot.revision().id(),
        "providerId":input.provider_id,"credentialId":input.credential_id,"result":result}),
    ))
}
fn revision_changed() -> GatewayError {
    GatewayError::conflict(
        "Configuration changed; refresh the saved account before reading a task.",
    )
    .with_code("console_revision_conflict")
}
async fn no_store(mut response: Response) -> Response {
    response.headers_mut().insert(
        axum::http::header::CACHE_CONTROL,
        axum::http::HeaderValue::from_static("no-store"),
    );
    response
}
