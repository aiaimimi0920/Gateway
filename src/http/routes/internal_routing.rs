use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::Json;
use serde::Deserialize;
use serde_json::Value;

use crate::db;
use crate::error::GatewayError;
use crate::http::extractors::OptionalBearerToken;
use crate::state::AppState;

use super::internal_gateway::assert_management_access;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoutePolicyListQuery {
    pub project_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelAliasListQuery {
    pub project_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoutePolicyBody {
    pub project_id: String,
    pub name: String,
    #[serde(default)]
    pub is_default: Option<bool>,
    #[serde(default)]
    pub enabled: Option<bool>,
    #[serde(default)]
    pub config: db::GatewayRoutePolicyConfigInput,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelAliasBody {
    #[serde(default)]
    pub project_id: Option<String>,
    #[serde(default)]
    pub scope_type: Option<String>,
    pub alias: String,
    pub provider_account_id: String,
    #[serde(default)]
    pub upstream_model: Option<String>,
    #[serde(default)]
    pub priority: Option<i32>,
    #[serde(default)]
    pub weight: Option<i32>,
    #[serde(default)]
    pub enabled: Option<bool>,
}

pub async fn list_route_policies(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Query(query): Query<RoutePolicyListQuery>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let route_policies =
        db::list_route_policies(required_pg_pool(state.as_ref())?, query.project_id.trim()).await?;
    Ok(Json(serde_json::json!({ "routePolicies": route_policies })))
}

pub async fn create_route_policy(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Json(body): Json<RoutePolicyBody>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let route_policy = save_route_policy_inner(state.as_ref(), None, body).await?;
    Ok(Json(serde_json::json!({ "routePolicy": route_policy })))
}

pub async fn update_route_policy(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Path(policy_id): Path<String>,
    Json(body): Json<RoutePolicyBody>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let route_policy =
        save_route_policy_inner(state.as_ref(), Some(policy_id.trim()), body).await?;
    Ok(Json(serde_json::json!({ "routePolicy": route_policy })))
}

pub async fn list_model_aliases(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Query(query): Query<ModelAliasListQuery>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let model_aliases = db::list_model_aliases(
        required_pg_pool(state.as_ref())?,
        query.project_id.as_deref(),
    )
    .await?;
    Ok(Json(serde_json::json!({ "modelAliases": model_aliases })))
}

pub async fn create_model_alias(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Json(body): Json<ModelAliasBody>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let model_alias = save_model_alias_inner(state.as_ref(), None, body).await?;
    Ok(Json(serde_json::json!({ "modelAlias": model_alias })))
}

pub async fn update_model_alias(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Path(alias_id): Path<String>,
    Json(body): Json<ModelAliasBody>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let model_alias = save_model_alias_inner(state.as_ref(), Some(alias_id.trim()), body).await?;
    Ok(Json(serde_json::json!({ "modelAlias": model_alias })))
}

pub async fn delete_model_alias(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Path(alias_id): Path<String>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let model_alias =
        db::delete_model_alias(required_pg_pool(state.as_ref())?, alias_id.trim()).await?;
    Ok(Json(serde_json::json!({
        "deleted": true,
        "modelAlias": model_alias
    })))
}

async fn save_route_policy_inner(
    state: &AppState,
    policy_id: Option<&str>,
    body: RoutePolicyBody,
) -> Result<db::GatewayRoutePolicyView, GatewayError> {
    let config = db::normalize_route_policy_config(body.config)?;
    db::save_route_policy(
        required_pg_pool(state)?,
        policy_id,
        db::SaveRoutePolicyInput {
            project_id: normalize_required_text(&body.project_id, "projectId", 120)?,
            name: normalize_required_text(&body.name, "name", 120)?,
            is_default: body.is_default.unwrap_or(false),
            enabled: body.enabled.unwrap_or(true),
            config,
        },
    )
    .await
}

async fn save_model_alias_inner(
    state: &AppState,
    alias_id: Option<&str>,
    body: ModelAliasBody,
) -> Result<db::GatewayModelAliasView, GatewayError> {
    db::save_model_alias(
        required_pg_pool(state)?,
        alias_id,
        db::SaveModelAliasInput {
            project_id: normalize_optional_text(body.project_id.as_deref(), 120),
            scope_type: normalize_model_alias_scope_type(body.scope_type.as_deref()),
            alias: normalize_required_text(&body.alias, "alias", 120)?,
            provider_account_id: normalize_required_text(
                &body.provider_account_id,
                "providerAccountId",
                120,
            )?,
            upstream_model: normalize_optional_text(body.upstream_model.as_deref(), 120),
            priority: normalize_non_negative_i32(body.priority, 100)?,
            weight: normalize_positive_i32(body.weight, 1)?,
            enabled: body.enabled.unwrap_or(true),
        },
    )
    .await
}

fn required_pg_pool(state: &AppState) -> Result<&sqlx::PgPool, GatewayError> {
    state
        .pg_pool
        .as_ref()
        .ok_or_else(|| GatewayError::service_unavailable("PostgreSQL 尚未配置"))
}

fn normalize_required_text(
    value: &str,
    label: &str,
    max_len: usize,
) -> Result<String, GatewayError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(GatewayError::bad_request(format!("{label} 不能为空")));
    }
    if trimmed.len() > max_len {
        return Err(GatewayError::bad_request(format!(
            "{label} 不能超过 {max_len} 个字符"
        )));
    }
    Ok(trimmed.to_string())
}

fn normalize_optional_text(value: Option<&str>, max_len: usize) -> Option<String> {
    let trimmed = value?.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(trimmed.chars().take(max_len).collect())
}

fn normalize_non_negative_i32(value: Option<i32>, default_value: i32) -> Result<i32, GatewayError> {
    match value.unwrap_or(default_value) {
        value if value < 0 => Err(GatewayError::bad_request("priority 必须是非负整数")),
        value => Ok(value),
    }
}

fn normalize_positive_i32(value: Option<i32>, default_value: i32) -> Result<i32, GatewayError> {
    match value.unwrap_or(default_value) {
        value if value <= 0 => Err(GatewayError::bad_request("weight 必须是正整数")),
        value => Ok(value),
    }
}

fn normalize_model_alias_scope_type(value: Option<&str>) -> String {
    match value.map(|raw| raw.trim().to_lowercase()) {
        Some(value) if value == "provider_special" => "provider_special".to_string(),
        _ => "global".to_string(),
    }
}
