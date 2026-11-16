//! Explicit signed inspection of a saved target/model. Never registers, claims, purchases or releases.
use super::{
    freebuff_catalog::{authorize_snapshot, failure, resolve, CatalogRequest, Client, Origin},
    json_with_no_store,
};
use crate::{
    error::GatewayError,
    http::extractors::OptionalBearerToken,
    protocol::freebuff::{
        business_target,
        catalog::{
            BusinessError, BusinessOrigin, CatalogSession, ClientSurface, SessionInspection,
        },
    },
    state::AppState,
};
use axum::{
    extract::{ConnectInfo, State},
    http::HeaderMap,
    response::Response,
    Json,
};
use serde::Deserialize;
use std::{net::SocketAddr, sync::Arc, time::Duration};
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InspectionRequest {
    provider_id: String,
    credential_id: String,
    expected_revision: String,
    model_key: String,
}
fn invalid() -> GatewayError {
    GatewayError::bad_request("FreeBuff inspection requires one exact saved account, business target and explicit saved model.").with_code("freebuff_inspection_target_invalid")
}
fn business_failure(error: BusinessError) -> GatewayError {
    GatewayError::service_unavailable(
        "FreeBuff session inspection failed; verify account, registered device and access.",
    )
    .with_code(error.to_string())
}
pub async fn inspect_freebuff_session(
    State(state): State<Arc<AppState>>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Json(body): Json<InspectionRequest>,
) -> Result<Response, GatewayError> {
    let (snapshot, fence) = authorize_snapshot(
        &state,
        connect_info,
        token.as_deref(),
        &headers,
        &body.expected_revision,
    )?;
    let provider = snapshot
        .document()
        .providers
        .iter()
        .find(|p| p.id == body.provider_id)
        .ok_or_else(invalid)?;
    let credential = provider
        .credentials
        .iter()
        .find(|c| c.id.as_deref() == Some(&body.credential_id))
        .ok_or_else(invalid)?;
    if !credential.supported_models.contains(&body.model_key)
        || !crate::credential_test_policy::exact_model(&body.model_key)
    {
        return Err(invalid());
    }
    let target = business_target::read(Some(&credential.extra_body))?.ok_or_else(invalid)?;
    let input = CatalogRequest {
        provider_id: body.provider_id,
        credential_id: body.credential_id,
        expected_revision: body.expected_revision,
        business_origin: match target.origin {
            BusinessOrigin::Codebuff => Origin::Codebuff,
            BusinessOrigin::CodebuffWww => Origin::Www,
        },
        client: match target.client {
            ClientSurface::Cli => Client::Cli,
            ClientSurface::Desktop => Client::Desktop,
        },
    };
    let token = resolve(&snapshot, &input)?;
    let local = state.local_runtime.as_ref().ok_or_else(|| {
        GatewayError::conflict("FreeBuff inspection requires the LocalRuntime device secret owner.")
            .with_code("freebuff_device_storage_unsupported")
    })?;
    let mut session = CatalogSession::new(target.origin, &token, target.client).map_err(failure)?;
    let device = local
        .freebuff_registration(&input.provider_id, &input.credential_id, &session)
        .await?
        .ok_or_else(|| {
            GatewayError::conflict(
                "Register this exact account and business target explicitly before inspection.",
            )
            .with_code("freebuff_device_registration_required")
        })?;
    fence()?;
    let _slot = crate::provider_discovery::background::SLOTS
        .try_acquire()
        .map_err(|_| GatewayError::conflict("Discovery is busy; try again shortly."))?;
    // One operation budget includes signed catalog, session body and local interpretation. No polling task.
    let inspection = tokio::time::timeout(Duration::from_secs(20), async {
        session
            .refresh(Some(&device), Duration::from_secs(10))
            .await
            .map_err(failure)?;
        fence()?;
        let reply = session
            .business_requests(&device, &body.model_key)
            .map_err(business_failure)?
            .inspect(Duration::from_secs(10))
            .await
            .map_err(business_failure)?;
        SessionInspection::parse(reply, &body.model_key, time::OffsetDateTime::now_utc())
            .map_err(business_failure)
    })
    .await
    .map_err(|_| business_failure(BusinessError::Timeout))??;
    fence()?;
    let mut response = serde_json::to_value(inspection)
        .map_err(|_| business_failure(BusinessError::InvalidResponse))?;
    for (name, value) in [
        ("revision", serde_json::json!(input.expected_revision)),
        ("providerId", serde_json::json!(input.provider_id)),
        ("credentialId", serde_json::json!(input.credential_id)),
        ("businessOrigin", serde_json::json!(input.business_origin)),
        ("client", serde_json::json!(input.client)),
        ("modelKey", serde_json::json!(body.model_key)),
    ] {
        response[name] = value;
    }
    Ok(json_with_no_store(response))
}
