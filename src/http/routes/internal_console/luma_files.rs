//! Secret-granted management only: resolve one saved account, never the public caller's namespace.
mod upload;
use super::{chatgpt_oauth::authorize, json_with_no_store};
use crate::{
    error::GatewayError,
    http::extractors::OptionalBearerToken,
    protocol::luma_agents::{files::Operation, PRESET_ID},
    state::AppState,
};
use axum::{
    extract::{ConnectInfo, Request, State},
    http::HeaderMap,
    response::Response,
    Router,
};
use serde::Deserialize;
use std::{net::SocketAddr, sync::Arc};

pub(crate) fn mount(router: Router<Arc<AppState>>) -> Router<Arc<AppState>> {
    router.route(
        "/v1/internal/gateway/console/luma-agents-files",
        axum::routing::post(manage)
            .layer(axum::extract::DefaultBodyLimit::max(
                crate::protocol::luma_agents::file_upload::MAX_UPLOAD + 16384,
            ))
            .layer(axum::middleware::map_response(no_store)),
    )
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Input {
    provider_id: String,
    credential_id: String,
    expected_revision: String,
    operation: Operation,
}

async fn manage(
    State(state): State<Arc<AppState>>,
    info: Option<ConnectInfo<SocketAddr>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    request: Request,
) -> Result<Response, GatewayError> {
    authorize(&state, &headers, token.as_deref(), info.as_ref(), true)?;
    if state.shutdown.is_requested() || state.lifecycle.is_draining() {
        return Err(GatewayError::service_unavailable("Gateway is stopping."));
    }
    // Admission precedes buffering base64 and holds through transport; concurrent uploads stay bounded.
    let _slot = crate::provider_discovery::background::SLOTS
        .try_acquire()
        .map_err(|_| GatewayError::conflict("Account operations are busy; try again shortly."))?;
    let input = tokio::time::timeout(std::time::Duration::from_secs(60), async {
        if headers
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .is_some_and(|s| s.split(';').next() == Some("multipart/form-data"))
        {
            upload::parse(request, &state).await
        } else {
            let bytes = axum::body::to_bytes(request.into_body(), 12 * 1024 * 1024)
                .await
                .map_err(|_| GatewayError::bad_request("Luma Files body exceeds the limit."))?;
            serde_json::from_slice::<Input>(&bytes)
                .map_err(|_| GatewayError::bad_request("Invalid Luma Files body."))
        }
    })
    .await
    .map_err(|_| GatewayError::bad_request("Luma Files request body timed out."))??;
    authorize(&state, &headers, token.as_deref(), info.as_ref(), true)?;
    let snapshot = state.route_config.snapshot();
    if snapshot.revision().id() != input.expected_revision {
        return Err(revision_changed());
    }
    let invalid = || {
        GatewayError::bad_request("Select a saved, enabled Luma Agents account.")
            .with_code("luma_files_account")
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
    let operation = input.operation.prepare()?;
    #[cfg(not(test))]
    let http = rquest::Client::builder()
        .build()
        .map_err(|_| GatewayError::service_unavailable("Luma Files transport unavailable."))?;
    #[cfg(test)]
    let http = state.upstream_client.http.clone();
    let result = Box::pin(crate::upstream::luma_agents::files::execute(
        &http,
        &target.payload,
        operation,
    ))
    .await?;
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
        "Configuration changed; refresh the saved account and file list before another write.",
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
