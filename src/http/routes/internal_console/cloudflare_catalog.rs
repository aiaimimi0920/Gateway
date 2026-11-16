//! Catalog reads bind the token to the pool Account ID; no caller-selected URLs or generation.
use super::{console_request_context, json_with_no_store, required_console_management_token};
use crate::{
    error::GatewayError, http::extractors::OptionalBearerToken,
    protocol::cloudflare_workers_ai as cf, provider_discovery::model_catalog, state::AppState,
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
pub struct CatalogRequest {
    credential_id: Option<String>,
    provider_id: Option<String>,
    api_key: Option<String>,
}

pub async fn read_cloudflare_model_catalog(
    State(state): State<Arc<AppState>>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Json(body): Json<CatalogRequest>,
) -> Result<Response, GatewayError> {
    let request = console_request_context(&headers, connect_info.as_ref());
    let actor = state.console_auth.authenticate_management_token(
        &request,
        required_console_management_token(token.as_deref(), &headers)?,
    )?;
    let grant = headers
        .get("x-secret-grant")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    state
        .console_auth
        .verify_secret_grant(&request, &actor, grant)?;
    if !cfg!(feature = "line-cloudflare-workers-ai") {
        return Err(
            GatewayError::conflict("Cloudflare Workers AI was compiled out.")
                .with_code("gateway_provider_line_compiled_out"),
        );
    }
    let _slot = crate::provider_discovery::background::SLOTS
        .try_acquire()
        .map_err(|_| GatewayError::conflict("Discovery is busy; try again shortly."))?;
    let snapshot = state.route_config.snapshot();
    let (base, key) = if let Some(id) = body.credential_id {
        if body.provider_id.is_some() || body.api_key.is_some() {
            return Err(cf::invalid());
        }
        let target = snapshot
            .select_credential_probe_target(&id)
            .ok_or_else(cf::invalid)?;
        let provider = snapshot
            .document()
            .providers
            .iter()
            .find(|p| {
                p.id == target.provider_id
                    && p.credentials.iter().any(|c| c.id.as_deref() == Some(&id))
            })
            .ok_or_else(cf::invalid)?;
        if provider.preset.as_deref() != Some(cf::PRESET)
            || cf::Endpoint::parse(&provider.base_url).map(|e| e.base_url)
                != cf::Endpoint::parse(&target.payload.base_url).map(|e| e.base_url)
        {
            return Err(cf::invalid());
        }
        cf::validate(&target.payload, &target.payload.adapter)?;
        (target.payload.base_url, target.payload.api_key)
    } else {
        let provider = snapshot
            .document()
            .providers
            .iter()
            .find(|p| {
                Some(p.id.as_str()) == body.provider_id.as_deref()
                    && p.preset.as_deref() == Some(cf::PRESET)
            })
            .ok_or_else(cf::invalid)?;
        (provider.base_url.clone(), body.api_key.unwrap_or_default())
    };
    let catalog = model_catalog::fetch_cloudflare_catalog(&base, &key).await?;
    state
        .console_auth
        .verify_secret_grant(&request, &actor, grant)?;
    if state.route_config.snapshot().revision().id() != snapshot.revision().id() {
        return Err(GatewayError::conflict(
            "Route configuration changed during catalog read; retry.",
        ));
    }
    catalog.validate(Some(cf::PRESET), &base).map_err(|_| {
        GatewayError::service_unavailable("Cloudflare directory does not match its account.")
    })?;
    Ok(json_with_no_store(
        serde_json::json!({"revision":snapshot.revision().id(),"catalog":catalog}),
    ))
}
