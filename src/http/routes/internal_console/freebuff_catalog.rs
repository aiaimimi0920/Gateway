//! Display-only catalog read for one exact stored account and explicitly selected business audience.
use super::{console_request_context, json_with_no_store, required_console_management_token};
use crate::{
    error::GatewayError,
    http::extractors::OptionalBearerToken,
    protocol::freebuff::catalog::{
        BusinessOrigin, CatalogError, CatalogSession, ClientSurface, ModelCatalog,
    },
    routing::config::{freebuff_session, RouteConfigSnapshot},
    state::AppState,
};
use axum::{
    extract::{ConnectInfo, State},
    http::HeaderMap,
    response::Response,
    Json,
};
use serde::{Deserialize, Serialize};
use std::{future::Future, net::SocketAddr, sync::Arc, time::Duration};

#[derive(Clone, Copy, Deserialize, Serialize)]
pub enum Origin {
    #[serde(rename = "https://codebuff.com")]
    Codebuff,
    #[serde(rename = "https://www.codebuff.com")]
    Www,
}
impl Origin {
    pub(super) fn protocol(self) -> BusinessOrigin {
        match self {
            Self::Codebuff => BusinessOrigin::Codebuff,
            Self::Www => BusinessOrigin::CodebuffWww,
        }
    }
}
#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Client {
    Cli,
    Desktop,
}
impl Client {
    pub(super) fn protocol(self) -> ClientSurface {
        match self {
            Self::Cli => ClientSurface::Cli,
            Self::Desktop => ClientSurface::Desktop,
        }
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CatalogRequest {
    pub(super) provider_id: String,
    pub(super) credential_id: String,
    pub(super) expected_revision: String,
    pub(super) business_origin: Origin,
    pub(super) client: Client,
}
fn invalid() -> GatewayError {
    GatewayError::bad_request("FreeBuff catalog requires an exact stored FreeBuff account with its original login audience.")
}
pub(super) fn resolve(
    snapshot: &RouteConfigSnapshot,
    input: &CatalogRequest,
) -> Result<String, GatewayError> {
    if !cfg!(feature = "line-freebuff-web-reverse-api") {
        return Err(
            GatewayError::conflict("FreeBuff implementation line was compiled out.")
                .with_code("gateway_provider_line_compiled_out"),
        );
    }
    let provider = snapshot
        .document()
        .providers
        .iter()
        .find(|p| p.id == input.provider_id)
        .ok_or_else(invalid)?;
    let credential = provider
        .credentials
        .iter()
        .find(|c| c.id.as_deref() == Some(&input.credential_id))
        .ok_or_else(invalid)?;
    let target = snapshot
        .select_credential_probe_target(&input.credential_id)
        .ok_or_else(invalid)?;
    if target.provider_id != provider.id {
        return Err(invalid());
    }
    // Do not relax typed login's freebuff.com binding, borrow another account, or execute generic discovery.
    freebuff_session::validate_target(provider, Some(credential), &target.payload)
        .map_err(|_| invalid())?;
    if target.payload.api_key.is_empty() {
        return Err(invalid());
    }
    Ok(target.payload.api_key)
}
pub(super) fn failure(error: CatalogError) -> GatewayError {
    let result = match error {
        CatalogError::InvalidInput => invalid(),
        CatalogError::FeatureDisabled => GatewayError::conflict("FreeBuff implementation line was compiled out."),
        _ => GatewayError::service_unavailable("FreeBuff catalog could not be read; check account access and the selected business site."),
    };
    result.with_code(error.to_string())
}
async fn fetch(
    local: Option<crate::local_runtime::LocalRuntime>,
    provider: String,
    credential: String,
    origin: Origin,
    client: Client,
    token: String,
) -> Result<Vec<serde_json::Value>, GatewayError> {
    let mut session =
        CatalogSession::new(origin.protocol(), &token, client.protocol()).map_err(failure)?;
    let device = match local {
        Some(local) => {
            local
                .freebuff_registration(&provider, &credential, &session)
                .await?
        }
        None => None,
    };
    let catalog = session
        .refresh(device.as_ref(), Duration::from_secs(10))
        .await
        .map_err(failure)?;
    // Read-only: restore an existing exact receipt, but never generate/register/repair a key or buy admission.
    Ok(display_rows(catalog))
}
pub async fn read_freebuff_model_catalog(
    State(state): State<Arc<AppState>>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Json(body): Json<CatalogRequest>,
) -> Result<Response, GatewayError> {
    let local = state.local_runtime.clone();
    let provider = body.provider_id.clone();
    let credential = body.credential_id.clone();
    read_with_fetch(
        state,
        connect_info,
        token,
        headers,
        body,
        move |origin, client, token| fetch(local, provider, credential, origin, client, token),
    )
    .await
}
// Private injection seam for post-fetch grant/revision races, not a runtime destination override.
async fn read_with_fetch<F, Fut>(
    state: Arc<AppState>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    token: Option<String>,
    headers: HeaderMap,
    input: CatalogRequest,
    fetch: F,
) -> Result<Response, GatewayError>
where
    F: FnOnce(Origin, Client, String) -> Fut,
    Fut: Future<Output = Result<Vec<serde_json::Value>, GatewayError>>,
{
    let (token, fence) = authorize(&state, connect_info, token.as_deref(), &headers, &input)?;
    let _slot = crate::provider_discovery::background::SLOTS
        .try_acquire()
        .map_err(|_| GatewayError::conflict("Discovery is busy; try again shortly."))?;
    let catalog = fetch(input.business_origin, input.client, token).await?;
    fence()?;
    Ok(json_with_no_store(
        serde_json::json!({"revision": input.expected_revision,
        "providerId":input.provider_id,"credentialId":input.credential_id,
        "businessOrigin":input.business_origin,"client":input.client,
        "models":catalog}),
    ))
}
// Shared management authority for read and explicit registration; the fence is not a network rollback.
pub(super) fn authorize<'a>(
    state: &'a AppState,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    token: Option<&str>,
    headers: &HeaderMap,
    input: &CatalogRequest,
) -> Result<(String, impl Fn() -> Result<(), GatewayError> + 'a), GatewayError> {
    let (snapshot, fence) = authorize_snapshot(
        state,
        connect_info,
        token,
        headers,
        &input.expected_revision,
    )?;
    Ok((resolve(&snapshot, input)?, fence))
}
// Pin authority and configuration once; saved-target operations must resolve from this same snapshot.
pub(super) fn authorize_snapshot<'a>(
    state: &'a AppState,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    token: Option<&str>,
    headers: &HeaderMap,
    expected_revision: &str,
) -> Result<
    (
        Arc<RouteConfigSnapshot>,
        impl Fn() -> Result<(), GatewayError> + 'a,
    ),
    GatewayError,
> {
    let request = console_request_context(headers, connect_info.as_ref());
    let actor = state.console_auth.authenticate_management_token(
        &request,
        required_console_management_token(token, headers)?,
    )?;
    let grant = headers
        .get("x-secret-grant")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_owned();
    state
        .console_auth
        .verify_secret_grant(&request, &actor, &grant)?;
    let snapshot = state.route_config.snapshot();
    if expected_revision != snapshot.revision().id() {
        return Err(GatewayError::conflict(
            "Route configuration changed; retry catalog read from the saved account.",
        ));
    }
    let pinned = snapshot.clone();
    Ok((pinned, move || {
        state
            .console_auth
            .verify_secret_grant(&request, &actor, &grant)?;
        if state.route_config.snapshot().revision().id() != snapshot.revision().id() {
            return Err(GatewayError::conflict("Route configuration changed during FreeBuff operation; retry from the saved account."));
        }
        Ok(())
    }))
}

fn display_rows(catalog: &ModelCatalog) -> Vec<serde_json::Value> {
    use crate::protocol::freebuff::catalog::{DataUse, ModelAccess};
    catalog.rows().map(|row| serde_json::json!({"key":row.key(), "displayName":row.display_name,
        "tagline":row.tagline,"warning":row.warning,"multimodal":row.multimodal,"premium":row.premium,
        "dataUse":match row.data_use { DataUse::Service => "service", DataUse::Training => "training" },
        "access":match row.access { ModelAccess::Open => "open", ModelAccess::Locked => "locked" },
        "badges":row.badges.iter().map(|b| serde_json::json!({"kind":b.kind,"label":b.label})).collect::<Vec<_>>()
    })).collect()
}
#[cfg(all(test, feature = "line-freebuff-web-reverse-api"))]
#[path = "freebuff_catalog_tests.rs"]
mod tests;

#[cfg(all(test, not(feature = "line-freebuff-web-reverse-api")))]
#[test]
fn freebuff_catalog_management_compiled_out_refuses_before_account_resolution() {
    let store = crate::routing::config::RouteConfigStore::new();
    let input = CatalogRequest {
        provider_id: "synthetic".into(),
        credential_id: "synthetic".into(),
        expected_revision: "synthetic".into(),
        business_origin: Origin::Codebuff,
        client: Client::Cli,
    };
    assert_eq!(
        resolve(&store.snapshot(), &input).unwrap_err().http_status,
        Some(409)
    );
}
