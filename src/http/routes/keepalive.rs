use std::sync::Arc;

use axum::extract::State;
use axum::http::HeaderMap;
use axum::Json;
use tracing::warn;

use crate::error::GatewayError;
use crate::http::extractors::OptionalBearerToken;
use crate::keepalive::{self, GatewayKeepaliveEnsureRequest, GatewayKeepaliveEnsureResponse};
use crate::state::AppState;

pub async fn ensure_credential_runtime(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Json(body): Json<GatewayKeepaliveEnsureRequest>,
) -> Result<Json<GatewayKeepaliveEnsureResponse>, GatewayError> {
    assert_keepalive_access(state.as_ref(), token.as_deref(), &headers)?;
    let response = keepalive::ensure_credential_runtime(
        &state.redis_pool,
        state.pg_pool.as_ref(),
        state.upstream_client.client(),
        body,
    )
    .await?;
    Ok(Json(response))
}

fn assert_keepalive_access(
    state: &AppState,
    bearer_token: Option<&str>,
    headers: &HeaderMap,
) -> Result<(), GatewayError> {
    let Some(expected) = state.config.gateway_keepalive_bearer_token.as_deref() else {
        warn!("GATEWAY_KEEPALIVE_BEARER_TOKEN is not set; allowing internal keepalive routes");
        return Ok(());
    };

    let provided = headers
        .get("x-internal-api-key")
        .and_then(|value| value.to_str().ok())
        .or_else(|| {
            headers
                .get("x-keepalive-token")
                .and_then(|value| value.to_str().ok())
        })
        .or(bearer_token)
        .map(str::trim)
        .filter(|value| !value.is_empty());

    match provided {
        Some(token) if token == expected => Ok(()),
        _ => Err(GatewayError::unauthorized("Keepalive token is required")),
    }
}
