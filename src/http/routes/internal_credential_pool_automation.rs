use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::Json;
use serde_json::Value;

use crate::credential_pool_automation::{
    run_provider_credential_pool_automation, run_provider_credential_pool_prune,
};
use crate::error::GatewayError;
use crate::http::extractors::OptionalBearerToken;
use crate::state::AppState;

use super::internal_gateway::assert_management_access;

pub async fn list_credential_pool_automation_status(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let snapshot = state.route_config.snapshot();
    let providers = state
        .credential_pool_automation
        .provider_views(&snapshot.document().providers)
        .await;
    Ok(Json(serde_json::json!({
        "automation": {
            "enabled": state.credential_pool_automation.enabled(),
            "intervalSeconds": state.credential_pool_automation.interval().as_secs(),
            "drivers": state.credential_pool_automation.drivers(),
            "providers": providers,
            "revisionId": snapshot.revision().id(),
        }
    })))
}

pub async fn run_credential_pool_automation_for_provider(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Path(provider_id): Path<String>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let result = run_provider_credential_pool_automation(&state, &provider_id)
        .await
        .map_err(|error| {
            GatewayError::bad_request(error.to_string())
                .with_code("credential_pool_automation_run_failed")
        })?;
    Ok(Json(serde_json::json!({ "provider": result })))
}

pub async fn prune_credential_pool_for_provider(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Path(provider_id): Path<String>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let result = run_provider_credential_pool_prune(&state, &provider_id)
        .await
        .map_err(|error| {
            GatewayError::bad_request(error.to_string()).with_code("credential_pool_prune_failed")
        })?;
    Ok(Json(serde_json::json!({ "provider": result })))
}

pub async fn purge_credential_pool_archive_for_provider(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Path(provider_id): Path<String>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let purged_count =
        crate::credential_pool_automation::archive_purge::purge_for_provider(&state, &provider_id)
            .await?;
    Ok(Json(serde_json::json!({ "purgedCount": purged_count })))
}
