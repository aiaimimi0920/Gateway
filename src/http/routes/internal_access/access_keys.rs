//! Management access-key lifecycle, balances and aggregate membership.

use super::super::internal_console::{console_request_context, required_console_management_token};
use super::super::internal_gateway::assert_management_access;
use super::required_pg_pool;
use crate::access_balance::AccessBalanceStore;
use crate::access_store::AccessStore;
use crate::db;
use crate::error::GatewayError;
use crate::http::extractors::OptionalBearerToken;
use crate::state::AppState;
use axum::extract::{ConnectInfo, Path, State};
use axum::http::HeaderMap;
use axum::Json;
use serde::Deserialize;
use std::sync::Arc;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccessKeyBody {
    pub owner_type: String,
    pub owner_id: String,
    pub resolved_project_id: String,
    pub resolved_tenant_id: String,
    pub key_kind: String,
    pub public_key_prefix: String,
    pub display_name: String,
    pub expires_at: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub quota: Option<crate::access_balance::quota::KeyQuotaInput>,
    #[serde(default)]
    pub bundle_ids: Vec<String>,
}

impl From<AccessKeyBody> for db::UpsertAccessKeyInput {
    fn from(body: AccessKeyBody) -> Self {
        Self {
            owner_type: body.owner_type,
            owner_id: body.owner_id,
            resolved_project_id: body.resolved_project_id,
            resolved_tenant_id: body.resolved_tenant_id,
            key_kind: body.key_kind,
            public_key_prefix: body.public_key_prefix,
            display_name: body.display_name,
            expires_at: body.expires_at,
            metadata: body.metadata,
            bundle_ids: body.bundle_ids,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RevokeAccessKeyBody {
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BalanceAdjustBody {
    pub balance_mode: Option<String>,
    pub status: Option<String>,
    pub unlimited_until: Option<String>,
    pub period_starts_at: Option<String>,
    pub period_ends_at: Option<String>,
    pub token_delta: Option<i64>,
    pub message_delta: Option<i64>,
    pub total_tokens: Option<i64>,
    pub remaining_tokens: Option<i64>,
    pub total_messages: Option<i64>,
    pub remaining_messages: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplaceAggregateMembershipsBody {
    pub memberships: Vec<AggregateMembershipBody>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AggregateMembershipBody {
    pub member_access_key_id: String,
    pub priority: i32,
}

#[derive(Debug, Deserialize)]
pub struct AccessKeyPath {
    pub access_key_id: String,
}

pub async fn create_access_key(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Json(mut body): Json<AccessKeyBody>,
) -> Result<Json<db::GatewayAccessKeyView>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let quota = body.quota.take();
    Ok(Json(
        AccessStore(&state)
            .save_with_quota(None, body.into(), quota.as_ref())
            .await?,
    ))
}

pub async fn update_access_key(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Path(path): Path<AccessKeyPath>,
    Json(mut body): Json<AccessKeyBody>,
) -> Result<Json<db::GatewayAccessKeyView>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let quota = body.quota.take();
    let mut key = AccessStore(&state)
        .save_with_quota(Some(&path.access_key_id), body.into(), quota.as_ref())
        .await?;
    key.token = None;
    key.external_key = None;
    Ok(Json(key))
}

pub async fn copy_access_key(
    State(state): State<Arc<AppState>>,
    connect_info: Option<ConnectInfo<std::net::SocketAddr>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Path(path): Path<AccessKeyPath>,
) -> Result<impl axum::response::IntoResponse, GatewayError> {
    let request = console_request_context(&headers, connect_info.as_ref());
    // Disclosure follows the console login/remote-access policy, never development auth bypass.
    state.console_auth.authenticate_management_token(
        &request,
        required_console_management_token(token.as_deref(), &headers)?,
    )?;
    let secret = AccessStore(&state).secret(&path.access_key_id).await?;
    Ok((
        [
            (axum::http::header::CACHE_CONTROL, "no-store"),
            (axum::http::header::PRAGMA, "no-cache"),
        ],
        Json(serde_json::json!({"token":secret})),
    ))
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccessKeyEnabledBody {
    enabled: bool,
}

pub async fn set_access_key_enabled(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Path(path): Path<AccessKeyPath>,
    Json(body): Json<AccessKeyEnabledBody>,
) -> Result<Json<serde_json::Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    AccessStore(&state)
        .set_enabled(&path.access_key_id, body.enabled)
        .await?;
    Ok(Json(serde_json::json!({ "success": true })))
}

pub async fn delete_access_key(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Path(path): Path<AccessKeyPath>,
) -> Result<Json<db::DeleteAccessKeyResult>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    Ok(Json(AccessStore(&state).delete(&path.access_key_id).await?))
}

pub async fn rotate_access_key(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Path(path): Path<AccessKeyPath>,
) -> Result<Json<db::GatewayAccessKeyView>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    Ok(Json(AccessStore(&state).rotate(&path.access_key_id).await?))
}

pub async fn revoke_access_key(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Path(path): Path<AccessKeyPath>,
    Json(body): Json<RevokeAccessKeyBody>,
) -> Result<Json<serde_json::Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    AccessStore(&state)
        .revoke(&path.access_key_id, body.reason.as_deref())
        .await?;
    Ok(Json(serde_json::json!({ "success": true })))
}

pub async fn adjust_access_key_balance(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Path(path): Path<AccessKeyPath>,
    Json(body): Json<BalanceAdjustBody>,
) -> Result<Json<db::GatewayAccessKeyBalanceView>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    Ok(Json(
        AccessBalanceStore::from_state(&state)?
            .adjust(
                &path.access_key_id,
                db::AccessKeyBalanceAdjustInput {
                    balance_mode: body.balance_mode,
                    status: body.status,
                    unlimited_until: body.unlimited_until,
                    period_starts_at: body.period_starts_at,
                    period_ends_at: body.period_ends_at,
                    token_delta: body.token_delta,
                    message_delta: body.message_delta,
                    total_tokens: body.total_tokens,
                    remaining_tokens: body.remaining_tokens,
                    total_messages: body.total_messages,
                    remaining_messages: body.remaining_messages,
                },
            )
            .await?,
    ))
}

pub async fn get_access_key_balance(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Path(path): Path<AccessKeyPath>,
) -> Result<Json<Option<db::GatewayAccessKeyBalanceView>>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    Ok(Json(
        AccessBalanceStore::from_state(&state)?
            .get(&path.access_key_id)
            .await?,
    ))
}

pub async fn replace_aggregate_memberships(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Path(path): Path<AccessKeyPath>,
    Json(body): Json<ReplaceAggregateMembershipsBody>,
) -> Result<Json<Vec<db::GatewayAccessKeyAggregateMembershipView>>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    Ok(Json(
        db::replace_access_key_aggregate_memberships(
            required_pg_pool(state.as_ref())?,
            &state.redis_pool,
            &path.access_key_id,
            &body
                .memberships
                .into_iter()
                .map(|value| db::AggregateMembershipInput {
                    member_access_key_id: value.member_access_key_id,
                    priority: value.priority,
                })
                .collect::<Vec<_>>(),
        )
        .await?,
    ))
}
