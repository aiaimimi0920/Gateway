//! Explicit local device registration; no automatic repair, uncertain replay, admission or model publication.
use super::{
    freebuff_catalog::{authorize, failure, CatalogRequest, Client, Origin},
    json_with_no_store,
};
use crate::{
    error::GatewayError,
    http::extractors::OptionalBearerToken,
    local_runtime::{
        freebuff_devices::{RegistrationIntent, RegistrationPreparation, RegistrationState},
        LocalRuntime,
    },
    protocol::freebuff::catalog::CatalogSession,
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
pub struct DeviceRequest {
    provider_id: String,
    credential_id: String,
    expected_revision: String,
    business_origin: Origin,
    client: Client,
    #[serde(default)]
    retry_unconfirmed_attempt_id: Option<String>,
    #[serde(default)]
    replace_stored_record_id: Option<String>,
}
pub async fn register_freebuff_device(
    State(state): State<Arc<AppState>>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Json(body): Json<DeviceRequest>,
) -> Result<Response, GatewayError> {
    let input = CatalogRequest {
        provider_id: body.provider_id,
        credential_id: body.credential_id,
        expected_revision: body.expected_revision,
        business_origin: body.business_origin,
        client: body.client,
    };
    let (token, fence) = authorize(&state, connect_info, token.as_deref(), &headers, &input)?;
    if (body.retry_unconfirmed_attempt_id.is_some() && body.replace_stored_record_id.is_some())
        || [
            &body.retry_unconfirmed_attempt_id,
            &body.replace_stored_record_id,
        ]
        .into_iter()
        .flatten()
        .any(|id| id.len() != 36 || uuid::Uuid::parse_str(id).is_err())
    {
        return Err(GatewayError::bad_request(
            "Registration requires one exact local record or uncertain attempt confirmation, never both.",
        ));
    }
    let intent = match (
        &body.retry_unconfirmed_attempt_id,
        &body.replace_stored_record_id,
    ) {
        (Some(id), _) => RegistrationIntent::Retry(id),
        (_, Some(id)) => RegistrationIntent::Replace(id),
        _ => RegistrationIntent::Register,
    };
    let local = owner(&state)?;
    let mut session = CatalogSession::new(
        input.business_origin.protocol(),
        &token,
        input.client.protocol(),
    )
    .map_err(failure)?;
    let existing = local
        .freebuff_registration_state(&input.provider_id, &input.credential_id, &session)
        .await?;
    if !intent.needs_send(&existing)? {
        fence()?;
        return Ok(reply(&input, existing));
    }
    let _slot = crate::provider_discovery::background::SLOTS
        .try_acquire()
        .map_err(|_| GatewayError::conflict("Discovery is busy; try again shortly."))?;
    session
        .refresh(None, Duration::from_secs(10))
        .await
        .map_err(failure)?;
    fence()?;
    let prepared = local
        .prepare_freebuff_registration(&input.provider_id, &input.credential_id, &session, intent)
        .await?;
    fence()?;
    match prepared {
        RegistrationPreparation::Existing(existing) => Ok(reply(&input, existing)),
        RegistrationPreparation::Send(attempt) => {
            // Marker is durable and the OS guard is held before POST. A dropped waiter leaves it uncertain.
            let device = session
                .register_device(attempt.key(), Duration::from_secs(10))
                .await
                .map_err(failure)?;
            fence()?;
            let record_id = attempt.complete(&session, &device).await?;
            // A verified receipt already committed here remains stored if the response fence fails.
            // Do not erase a known result or re-POST it; a later exact-account read reconciles it.
            #[cfg(all(test, feature = "line-freebuff-web-reverse-api"))]
            let _ = AFTER_SAVE.try_with(|hook| hook());
            fence()?;
            Ok(reply(&input, RegistrationState::Stored(record_id)))
        }
    }
}
fn owner(state: &AppState) -> Result<&LocalRuntime, GatewayError> {
    state.local_runtime.as_ref().ok_or_else(|| GatewayError::conflict("FreeBuff durable device registration requires the selected LocalRuntime secret owner; Server mode is not supported yet.").with_code("freebuff_device_storage_unsupported"))
}
// No discovery slot, GET/POST or key creation: unknown outcomes remain inspectable during upstream failure.
pub async fn read_freebuff_device_state(
    State(state): State<Arc<AppState>>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Json(input): Json<CatalogRequest>,
) -> Result<Response, GatewayError> {
    let (token, fence) = authorize(&state, connect_info, token.as_deref(), &headers, &input)?;
    let local = owner(&state)?;
    let session = CatalogSession::new(
        input.business_origin.protocol(),
        &token,
        input.client.protocol(),
    )
    .map_err(failure)?;
    let result = local
        .freebuff_registration_state(&input.provider_id, &input.credential_id, &session)
        .await?;
    fence()?;
    Ok(reply(&input, result))
}

#[cfg(all(test, feature = "line-freebuff-web-reverse-api"))]
tokio::task_local! { pub(super) static AFTER_SAVE: Arc<dyn Fn() + Send + Sync>; }

#[cfg(all(test, not(feature = "line-freebuff-web-reverse-api")))]
#[path = "freebuff_device_disabled_tests.rs"]
mod disabled;
fn reply(input: &CatalogRequest, state: RegistrationState) -> Response {
    let mut value = serde_json::json!({"revision":input.expected_revision,"providerId":input.provider_id,
        "credentialId":input.credential_id,"businessOrigin":input.business_origin,"client":input.client});
    match state {
        RegistrationState::Absent => value["registration"] = serde_json::json!("absent"),
        RegistrationState::Stored(id) => {
            value["registration"] = serde_json::json!("stored");
            value["recordId"] = serde_json::json!(id);
        }
        RegistrationState::Unconfirmed(id) => {
            value["registration"] = serde_json::json!("unconfirmed");
            value["unconfirmedAttemptId"] = serde_json::json!(id);
        }
    }
    json_with_no_store(value)
}
