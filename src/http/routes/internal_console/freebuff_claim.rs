//! Explicit saved-account local journal and original-attempt end. No purchase or active adoption.
mod end;
use super::{
    freebuff_catalog::{authorize_snapshot, failure, resolve, CatalogRequest, Client, Origin},
    json_with_no_store,
};
use crate::{
    error::GatewayError,
    http::extractors::OptionalBearerToken,
    local_runtime::LocalRuntime,
    protocol::freebuff::{
        business_target,
        catalog::{BusinessError, BusinessOrigin, CatalogSession, ClientSurface},
    },
    state::AppState,
};
use axum::{
    extract::{ConnectInfo, State},
    http::HeaderMap,
    response::Response,
    Json,
};
pub use end::end_freebuff_claim;
use serde::Deserialize;
use serde_json::Value;
use std::{net::SocketAddr, sync::Arc};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct JournalRequest {
    provider_id: String,
    credential_id: String,
    expected_revision: String,
    #[serde(default)]
    cursor: Option<String>,
}
struct SavedAccount {
    input: CatalogRequest,
    session: CatalogSession,
    local: LocalRuntime,
}
fn invalid() -> GatewayError {
    GatewayError::bad_request(
        "FreeBuff claim operations require one exact saved account and its saved business target.",
    )
    .with_code("freebuff_session_claim_target_invalid")
}
fn attempt(value: &str) -> Result<uuid::Uuid, GatewayError> {
    uuid::Uuid::parse_str(value)
        .ok()
        .filter(|id| id.get_version_num() == 4 && id.to_string() == value)
        .ok_or_else(invalid)
}
fn prepare<'a>(
    state: &'a AppState,
    connect: Option<ConnectInfo<SocketAddr>>,
    token: Option<&str>,
    headers: &HeaderMap,
    provider_id: &str,
    credential_id: &str,
    revision: &str,
) -> Result<(SavedAccount, impl Fn() -> Result<(), GatewayError> + 'a), GatewayError> {
    let (snapshot, fence) = authorize_snapshot(state, connect, token, headers, revision)?;
    let credential = snapshot
        .document()
        .providers
        .iter()
        .find(|provider| provider.id == provider_id)
        .and_then(|provider| {
            provider
                .credentials
                .iter()
                .find(|credential| credential.id.as_deref() == Some(credential_id))
        })
        .ok_or_else(invalid)?;
    let target = business_target::read(Some(&credential.extra_body))?.ok_or_else(invalid)?;
    let input = CatalogRequest {
        provider_id: provider_id.into(),
        credential_id: credential_id.into(),
        expected_revision: revision.into(),
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
    let local = state.local_runtime.clone().ok_or_else(|| {
        GatewayError::conflict(
            "FreeBuff claim journal requires the selected LocalRuntime secret owner.",
        )
        .with_code("freebuff_session_claim_storage_unsupported")
    })?;
    let session = CatalogSession::new(target.origin, &token, target.client).map_err(failure)?;
    Ok((
        SavedAccount {
            input,
            session,
            local,
        },
        fence,
    ))
}
fn reply(input: &CatalogRequest, mut value: Value) -> Response {
    for (key, data) in [
        ("revision", serde_json::json!(input.expected_revision)),
        ("providerId", serde_json::json!(input.provider_id)),
        ("credentialId", serde_json::json!(input.credential_id)),
        ("businessOrigin", serde_json::json!(input.business_origin)),
        ("client", serde_json::json!(input.client)),
    ] {
        value[key] = data;
    }
    json_with_no_store(value)
}
fn journal_error() -> GatewayError {
    GatewayError::service_unavailable(
        "FreeBuff journal could not be interpreted; original records were retained.",
    )
    .with_code("freebuff_session_claim_unavailable")
}
fn end_error(error: BusinessError) -> GatewayError {
    GatewayError::service_unavailable("FreeBuff original-claim end is unconfirmed. Check the local claim and closeout history; do not automatically replay.")
        .with_code(error.to_string())
}
pub async fn read_freebuff_claim(
    State(state): State<Arc<AppState>>,
    connect: Option<ConnectInfo<SocketAddr>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Json(body): Json<JournalRequest>,
) -> Result<Response, GatewayError> {
    let (account, fence) = prepare(
        &state,
        connect,
        token.as_deref(),
        &headers,
        &body.provider_id,
        &body.credential_id,
        &body.expected_revision,
    )?;
    let cursor = body.cursor.as_deref().map(attempt).transpose()?;
    fence()?;
    let journal = account
        .local
        .freebuff_claim_journal(
            &body.provider_id,
            &body.credential_id,
            &account.session.stored_claim_scope(),
            cursor,
        )
        .await?;
    fence()?;
    let mut value = serde_json::to_value(journal).map_err(|_| journal_error())?;
    value["cursor"] = serde_json::json!(body.cursor);
    Ok(reply(&account.input, value))
}

#[cfg(all(test, not(feature = "line-freebuff-web-reverse-api")))]
mod disabled_tests;
#[cfg(all(test, feature = "line-freebuff-web-reverse-api"))]
mod tests;
