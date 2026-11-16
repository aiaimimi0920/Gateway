//! A saved-account directory read is fenced by management consent and revision, never a write.
use super::{chatgpt_oauth::authorize, json_with_no_store};
use crate::{
    error::GatewayError,
    http::extractors::OptionalBearerToken,
    protocol::dim_oauth::{business, AuthorizationContext},
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
use std::{net::SocketAddr, sync::Arc};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CatalogRequest {
    provider_id: String,
    credential_id: String,
    expected_revision: String,
}
fn resolve(
    snapshot: &RouteConfigSnapshot,
    input: &CatalogRequest,
) -> Result<(AuthorizationContext, String), GatewayError> {
    let target = snapshot
        .select_credential_probe_target(&input.credential_id)
        .ok_or_else(business::invalid)?;
    if target.provider_id != input.provider_id {
        return Err(business::invalid());
    }
    let provider = snapshot
        .document()
        .providers
        .iter()
        .find(|p| p.id == input.provider_id && p.explicit_credentials_only)
        .ok_or_else(business::invalid)?;
    let credential = provider
        .credentials
        .iter()
        .find(|c| c.id.as_deref() == Some(input.credential_id.as_str()))
        .ok_or_else(business::invalid)?;
    let context = credential
        .dim_oauth
        .as_ref()
        .ok_or_else(business::invalid)?;
    business::validate_context(&target.payload, context)?;
    if provider.base_url.trim_end_matches('/') != business::base(context.site())
        || !business::valid_token(&target.payload.api_key)
    {
        return Err(business::invalid());
    }
    Ok((context.clone(), target.payload.api_key))
}
pub async fn read_dim_model_catalog(
    State(state): State<Arc<AppState>>,
    info: Option<ConnectInfo<SocketAddr>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Json(input): Json<CatalogRequest>,
) -> Result<Response, GatewayError> {
    authorize(&state, &headers, token.as_deref(), info.as_ref(), true)?;
    if !cfg!(feature = "line-dim-user-oauth") {
        return Err(
            GatewayError::conflict("DIM OAuth implementation line was compiled out.")
                .with_code("gateway_provider_line_compiled_out"),
        );
    }
    let snapshot = state.route_config.snapshot();
    if snapshot.revision().id() != input.expected_revision {
        return Err(revision_changed());
    }
    let (context, access) = resolve(&snapshot, &input)?;
    let _slot = crate::provider_discovery::background::SLOTS
        .try_acquire()
        .map_err(|_| GatewayError::conflict("Discovery is busy; try again shortly."))?;
    // Context fixes authority; transport already bounds decoded bytes/time and rejects redirects.
    let catalog = context
        .model_catalog(state.upstream_client.client(), &access)
        .await
        .map_err(|_| {
            GatewayError::service_unavailable("DIM model directory read failed.")
                .with_code("dim_catalog_read_failed")
        })?;
    authorize(&state, &headers, token.as_deref(), info.as_ref(), true)?;
    if state.route_config.snapshot().revision().id() != snapshot.revision().id() {
        return Err(revision_changed());
    }
    Ok(json_with_no_store(
        serde_json::json!({"revision":snapshot.revision().id(),
        "providerId":input.provider_id,"credentialId":input.credential_id,"catalog":catalog}),
    ))
}
fn revision_changed() -> GatewayError {
    GatewayError::conflict(
        "Route configuration changed during DIM directory read; retry from the saved account.",
    )
    .with_code("console_revision_conflict")
}
