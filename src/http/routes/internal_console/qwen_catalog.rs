//! Native Qwen directory metadata. Stored secrets never accept a caller-supplied destination.
use super::{console_request_context, json_with_no_store, required_console_management_token};
use crate::{
    error::GatewayError,
    http::extractors::OptionalBearerToken,
    protocol::qwen::workspace_api,
    provider_discovery::model_catalog::{self, qwen_endpoint::QwenEndpoint, ModelCatalog},
    routing::config::RouteConfigSnapshot,
    state::AppState,
};
use axum::{
    extract::{ConnectInfo, State},
    http::HeaderMap,
    response::Response,
    Json,
};
use serde::Deserialize;
use std::{future::Future, net::SocketAddr, sync::Arc};

#[derive(Deserialize)]
#[serde(untagged)]
pub enum CatalogRequest {
    Stored(StoredAccount),
    Draft(DraftAccount),
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StoredAccount {
    credential_id: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DraftAccount {
    provider_id: String,
    account_base_url: String,
    api_key: String,
}

fn invalid() -> GatewayError {
    GatewayError::bad_request(
        "Qwen catalog requires an explicit ordinary workspace API account and matching Key.",
    )
}
fn resolve(
    snapshot: &RouteConfigSnapshot,
    body: CatalogRequest,
) -> Result<(QwenEndpoint, String), GatewayError> {
    if !cfg!(feature = "line-qwen-official-api") {
        return Err(GatewayError::conflict(
            "Qwen workspace API implementation line was compiled out.",
        )
        .with_code("gateway_provider_line_compiled_out"));
    }
    let (provider_id, base, key) = match body {
        CatalogRequest::Stored(account) => {
            let target = snapshot
                .select_credential_probe_target(&account.credential_id)
                .ok_or_else(invalid)?;
            let provider = snapshot
                .document()
                .providers
                .iter()
                .find(|p| {
                    p.id == target.provider_id
                        && p.credentials
                            .iter()
                            .any(|c| c.id.as_deref() == Some(&account.credential_id))
                })
                .ok_or_else(invalid)?;
            workspace_api::validate(&target.payload, &target.payload.adapter)?;
            (
                provider.id.as_str(),
                target.payload.base_url,
                target.payload.api_key,
            )
        }
        CatalogRequest::Draft(account) => {
            let provider = snapshot
                .document()
                .providers
                .iter()
                .find(|p| p.id == account.provider_id)
                .ok_or_else(invalid)?;
            // The Key is explicitly new material. Only the descriptor grammar, not an arbitrary URL,
            // chooses its destination; no stored secret is borrowed for a new workspace.
            (
                provider.id.as_str(),
                account.account_base_url,
                account.api_key,
            )
        }
    };
    let provider = snapshot
        .document()
        .providers
        .iter()
        .find(|p| p.id == provider_id)
        .ok_or_else(invalid)?;
    if provider.preset.as_deref() != Some("qwen-official-api") {
        return Err(invalid());
    }
    let endpoint = QwenEndpoint::parse(&base).ok_or_else(invalid)?;
    if key.is_empty() || key.len() > 16384 || !key.bytes().all(|b| b.is_ascii_graphic()) {
        return Err(invalid());
    }
    Ok((endpoint, key))
}

pub async fn read_qwen_model_catalog(
    State(state): State<Arc<AppState>>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Json(body): Json<CatalogRequest>,
) -> Result<Response, GatewayError> {
    read_with_fetch(
        state,
        connect_info,
        token,
        headers,
        body,
        |base, key| async move { model_catalog::fetch_qwen_catalog(&base, &key).await },
    )
    .await
}

// Private injection seam permits synthetic lifecycle tests, never a runtime transport override.
async fn read_with_fetch<F, Fut>(
    state: Arc<AppState>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    token: Option<String>,
    headers: HeaderMap,
    body: CatalogRequest,
    fetch: F,
) -> Result<Response, GatewayError>
where
    F: FnOnce(String, String) -> Fut,
    Fut: Future<Output = Result<ModelCatalog, GatewayError>>,
{
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
    let _slot = crate::provider_discovery::background::SLOTS
        .try_acquire()
        .map_err(|_| GatewayError::conflict("Discovery is busy; try again shortly."))?;
    let snapshot = state.route_config.snapshot();
    let (endpoint, key) = resolve(&snapshot, body)?;
    let catalog = fetch(endpoint.base_url.clone(), key).await?;
    // Grant/revision checks are read fences, not persistence CAS or account entitlement.
    state
        .console_auth
        .verify_secret_grant(&request, &actor, grant)?;
    if state.route_config.snapshot().revision().id() != snapshot.revision().id() {
        return Err(GatewayError::conflict(
            "Route configuration changed during catalog read; retry.",
        ));
    }
    catalog
        .validate(Some("qwen-official-api"), &endpoint.base_url)
        .map_err(|_| {
            GatewayError::service_unavailable(
                "Qwen catalog response does not match account target.",
            )
        })?;
    Ok(json_with_no_store(
        serde_json::json!({"revision":snapshot.revision().id(), "catalog":catalog}),
    ))
}

#[cfg(all(test, feature = "line-qwen-official-api"))]
#[path = "qwen_catalog_tests.rs"]
mod tests;
