//! Decisions only notify an existing live request. No quote generation, purchase or executor here.
use super::{
    freebuff_catalog::{authorize_snapshot, resolve, CatalogRequest, Client, Origin},
    json_with_no_store,
};
use crate::{
    console::freebuff_purchase::Fence,
    error::GatewayError,
    http::extractors::OptionalBearerToken,
    protocol::freebuff::{
        business_target,
        catalog::{BusinessOrigin, ClientSurface, PurchaseContext},
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
use std::{net::SocketAddr, sync::Arc};
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReadInput {
    provider_id: String,
    credential_id: String,
    expected_revision: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum Decision {
    Approve,
    Cancel,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DecisionInput {
    provider_id: String,
    credential_id: String,
    expected_revision: String,
    pending_id: String,
    quote_id: String,
    decision: Decision,
}
fn authorize(
    state: &AppState,
    connect: Option<ConnectInfo<SocketAddr>>,
    token: Option<&str>,
    headers: &HeaderMap,
    input: &ReadInput,
) -> Result<PurchaseContext, GatewayError> {
    let (snapshot, fence) =
        authorize_snapshot(state, connect, token, headers, &input.expected_revision)?;
    let invalid = || {
        GatewayError::bad_request("FreeBuff pending decisions require the original exact saved account and business target.")
        .with_code("freebuff_purchase_pending_target_invalid")
    };
    let credential = snapshot
        .document()
        .providers
        .iter()
        .find(|p| p.id == input.provider_id)
        .and_then(|p| {
            p.credentials
                .iter()
                .find(|c| c.id.as_deref() == Some(&input.credential_id))
        })
        .ok_or_else(invalid)?;
    let target = business_target::read(Some(&credential.extra_body))?.ok_or_else(invalid)?;
    resolve(
        &snapshot,
        &CatalogRequest {
            provider_id: input.provider_id.clone(),
            credential_id: input.credential_id.clone(),
            expected_revision: input.expected_revision.clone(),
            business_origin: match target.origin {
                BusinessOrigin::Codebuff => Origin::Codebuff,
                BusinessOrigin::CodebuffWww => Origin::Www,
            },
            client: match target.client {
                ClientSurface::Cli => Client::Cli,
                ClientSurface::Desktop => Client::Desktop,
            },
        },
    )?;
    fence()?;
    PurchaseContext::new(
        &input.provider_id,
        &input.credential_id,
        &input.expected_revision,
    )
    .map_err(|_| invalid())
}
pub async fn read_freebuff_pending_purchases(
    State(state): State<Arc<AppState>>,
    connect: Option<ConnectInfo<SocketAddr>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Json(input): Json<ReadInput>,
) -> Result<Response, GatewayError> {
    let context = authorize(&state, connect, token.as_deref(), &headers, &input)?;
    let pending = state.lifecycle.freebuff_purchases.list(&context);
    authorize(&state, connect, token.as_deref(), &headers, &input)?;
    Ok(json_with_no_store(
        serde_json::json!({"revision":input.expected_revision,
        "providerId":input.provider_id,"credentialId":input.credential_id,"pending":pending}),
    ))
}
pub async fn decide_freebuff_purchase(
    State(state): State<Arc<AppState>>,
    connect: Option<ConnectInfo<SocketAddr>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Json(input): Json<DecisionInput>,
) -> Result<Response, GatewayError> {
    let account = ReadInput {
        provider_id: input.provider_id,
        credential_id: input.credential_id,
        expected_revision: input.expected_revision,
    };
    let context = authorize(&state, connect, token.as_deref(), &headers, &account)?;
    let runtime = state.clone();
    // Reauthenticate the CURRENT management token, not just a captured actor/grant. This catches
    // management-key revocation after delivery, plus Grant expiry and configuration changes.
    let fence: Fence = Arc::new(move || {
        authorize(&runtime, connect, token.as_deref(), &headers, &account).map(|_| ())
    });
    let parse = |value: &str| {
        Uuid::parse_str(value)
            .ok()
            .filter(|id| id.get_version_num() == 4)
            .ok_or_else(|| {
                GatewayError::bad_request(
                    "FreeBuff pending and quote IDs must be host-issued UUIDs.",
                )
            })
    };
    state.lifecycle.freebuff_purchases.decide(
        &context,
        parse(&input.pending_id)?,
        parse(&input.quote_id)?,
        matches!(input.decision, Decision::Approve),
        fence,
    )?;
    Ok(json_with_no_store(serde_json::json!({"delivered":true})))
}
#[cfg(all(test, not(feature = "line-freebuff-web-reverse-api")))]
mod disabled_tests;
