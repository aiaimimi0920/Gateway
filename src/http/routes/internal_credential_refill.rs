use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::Json;
use serde_json::Value;

use crate::credential_refill::{
    claim_credential_refill_task, complete_credential_refill_task,
    create_user_requested_refill_task, fail_credential_refill_task, list_credential_refill_demands,
    list_credential_refill_tasks, renew_credential_refill_task, stream_key,
    ClaimCredentialRefillTaskInput, CompleteCredentialRefillTaskInput,
    CreateCredentialRefillRequestInput, CredentialRefillTaskFilters, FailCredentialRefillTaskInput,
    RenewCredentialRefillTaskInput,
};
use crate::error::GatewayError;
use crate::http::extractors::OptionalBearerToken;
use crate::state::AppState;

use super::internal_gateway::assert_management_access;

pub async fn get_credential_refill_status(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let demands = list_credential_refill_demands(state.as_ref()).await?;
    let tasks = list_credential_refill_tasks(
        state.as_ref(),
        CredentialRefillTaskFilters {
            limit: Some(50),
            ..CredentialRefillTaskFilters::default()
        },
    )
    .await?;
    Ok(Json(serde_json::json!({
        "refill": {
            "enabled": state.credential_pool_automation.refill_queue_enabled(),
            "streamKey": stream_key(),
            "notificationIntervalSeconds": state
                .credential_pool_automation
                .refill_notification_interval()
                .as_secs(),
            "defaultLeaseSeconds": state
                .credential_pool_automation
                .refill_default_lease_seconds(),
            "maxLeaseSeconds": state
                .credential_pool_automation
                .refill_max_lease_seconds(),
            "revisionId": state.route_config.snapshot().revision().id(),
            "providers": demands,
            "recentTasks": tasks,
        }
    })))
}

pub async fn list_credential_refill_tasks_route(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Query(filters): Query<CredentialRefillTaskFilters>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let tasks = list_credential_refill_tasks(state.as_ref(), filters).await?;
    Ok(Json(serde_json::json!({ "tasks": tasks })))
}

pub async fn request_credential_refill_for_provider(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Path(provider_id): Path<String>,
    Json(input): Json<CreateCredentialRefillRequestInput>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let result = create_user_requested_refill_task(state.as_ref(), &provider_id, input).await?;
    Ok(Json(serde_json::json!({
        "task": result.task,
        "created": result.created,
    })))
}

pub async fn claim_credential_refill_task_route(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Json(input): Json<ClaimCredentialRefillTaskInput>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let result = claim_credential_refill_task(state.as_ref(), input).await?;
    Ok(Json(serde_json::json!({
        "task": result.task,
        "claimToken": result.claim_token,
    })))
}

pub async fn renew_credential_refill_task_route(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Path(task_id): Path<String>,
    Json(input): Json<RenewCredentialRefillTaskInput>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let task = renew_credential_refill_task(state.as_ref(), &task_id, input).await?;
    Ok(Json(serde_json::json!({ "task": task })))
}

pub async fn complete_credential_refill_task_route(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Path(task_id): Path<String>,
    Json(input): Json<CompleteCredentialRefillTaskInput>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let task = complete_credential_refill_task(&state, &task_id, input).await?;
    Ok(Json(serde_json::json!({ "task": task })))
}

pub async fn fail_credential_refill_task_route(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Path(task_id): Path<String>,
    Json(input): Json<FailCredentialRefillTaskInput>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let task = fail_credential_refill_task(state.as_ref(), &task_id, input).await?;
    Ok(Json(serde_json::json!({ "task": task })))
}
