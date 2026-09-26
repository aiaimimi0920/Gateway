//! Management project API-key issuance, rotation and benefit-project setup.

use super::project_records::{
    format_timestamp, load_active_project_access_key, load_project_detail, load_tenant_detail,
    project_view_from_row, tenant_view_from_row,
};
use super::{assert_management_access, required_pg_pool};
use crate::db;
use crate::error::GatewayError;
use crate::gateway_api_key::build_gateway_project_api_key;
use crate::http::extractors::OptionalBearerToken;
use crate::state::AppState;
use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::Json;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Deserialize)]
pub struct ResolveApiAccessBody {
    #[serde(alias = "projectId")]
    pub project_id: String,
    pub name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RotateApiAccessBody {
    #[serde(alias = "projectId")]
    pub project_id: String,
    pub name: Option<String>,
    #[serde(default, alias = "actorUserId")]
    pub actor_user_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RotateProjectApiAccessBody {
    pub name: Option<String>,
    #[serde(default, alias = "actorUserId")]
    pub actor_user_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct EnsureBenefitProjectBody {
    #[serde(alias = "serviceId")]
    pub service_id: String,
    #[serde(alias = "userId")]
    pub user_id: String,
    #[serde(default, alias = "serviceTitle")]
    pub service_title: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ProjectPath {
    pub project_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayApiAccessResponse {
    pub project_id: String,
    pub tenant_id: String,
    pub project: db::GatewayProjectView,
    pub tenant: db::GatewayTenantView,
    pub api_key: db::GatewayApiKeyView,
    pub token: String,
}

async fn issue_or_get_project_access_key(
    state: &AppState,
    project_id: &str,
    name: &str,
) -> Result<GatewayApiAccessResponse, GatewayError> {
    let pg_pool = required_pg_pool(state)?;
    let secret = state
        .config
        .gateway_api_key_secret
        .as_deref()
        .ok_or_else(|| GatewayError::conflict("当前环境尚未配置 GATEWAY_API_KEY_SECRET"))?;
    let project_row = load_project_detail(pg_pool, project_id).await?;
    let tenant_row = load_tenant_detail(pg_pool, &project_row.tenant_id).await?;
    let _ = db::ensure_default_access_bundle_for_project(
        pg_pool,
        &state.redis_pool,
        project_id,
        &format!("{} 默认访问包", project_row.display_name),
    )
    .await?;
    let access_key_view =
        if let Some(existing) = load_active_project_access_key(pg_pool, project_id).await? {
            db::GatewayAccessKeyView {
                id: existing.id.clone(),
                owner_type: "project".to_string(),
                owner_id: project_id.to_string(),
                resolved_project_id: existing.resolved_project_id.clone(),
                resolved_tenant_id: existing.resolved_tenant_id.clone(),
                key_kind: "normal".to_string(),
                status: existing.status.clone(),
                public_key_prefix: "neuro".to_string(),
                display_name: existing.display_name.clone(),
                token: Some(build_gateway_project_api_key(
                    &existing.id,
                    &existing.resolved_project_id,
                    &existing.resolved_tenant_id,
                    secret,
                )),
                external_key: None,
                rotated_from_access_key_id: existing.rotated_from_access_key_id.clone(),
                legacy_gateway_api_key_id: None,
                legacy_user_credential_id: None,
                expires_at: None,
                last_used_at: None,
                metadata: None,
                revoked_at: existing.revoked_at.map(format_timestamp),
                revoke_reason: None,
                created_at: format_timestamp(existing.created_at),
                updated_at: format_timestamp(existing.created_at),
            }
        } else {
            db::save_access_key(
                pg_pool,
                &state.redis_pool,
                None,
                Some(secret),
                db::UpsertAccessKeyInput {
                    owner_type: "project".to_string(),
                    owner_id: project_id.to_string(),
                    resolved_project_id: project_id.to_string(),
                    resolved_tenant_id: tenant_row.id.clone(),
                    key_kind: "normal".to_string(),
                    public_key_prefix: "neuro".to_string(),
                    display_name: name.to_string(),
                    expires_at: None,
                    metadata: None,
                    bundle_ids: Vec::new(),
                },
            )
            .await?
        };
    Ok(GatewayApiAccessResponse {
        project_id: project_row.id.clone(),
        tenant_id: tenant_row.id.clone(),
        project: project_view_from_row(project_row),
        tenant: tenant_view_from_row(tenant_row),
        api_key: db::GatewayApiKeyView {
            id: access_key_view.id.clone(),
            project_id: access_key_view.resolved_project_id.clone(),
            name: access_key_view.display_name.clone(),
            status: access_key_view.status.clone(),
            issued_at: access_key_view.created_at.clone(),
            revoked_at: access_key_view.revoked_at.clone(),
            rotated_from_api_key_id: access_key_view.rotated_from_access_key_id.clone(),
        },
        token: access_key_view.token.clone().unwrap_or_else(|| {
            build_gateway_project_api_key(
                &access_key_view.id,
                &access_key_view.resolved_project_id,
                &access_key_view.resolved_tenant_id,
                secret,
            )
        }),
    })
}

pub async fn resolve_api_access(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Json(body): Json<ResolveApiAccessBody>,
) -> Result<Json<GatewayApiAccessResponse>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    Ok(Json(
        issue_or_get_project_access_key(
            state.as_ref(),
            body.project_id.trim(),
            body.name.as_deref().unwrap_or("benefit-project-key"),
        )
        .await?,
    ))
}

pub async fn rotate_api_access(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Json(body): Json<RotateApiAccessBody>,
) -> Result<Json<GatewayApiAccessResponse>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let pg_pool = required_pg_pool(state.as_ref())?;
    if let Some(existing) = load_active_project_access_key(pg_pool, body.project_id.trim()).await? {
        let _ = db::rotate_access_key(
            pg_pool,
            &state.redis_pool,
            &existing.id,
            state.config.gateway_api_key_secret.as_deref(),
        )
        .await?;
    }
    Ok(Json(
        issue_or_get_project_access_key(
            state.as_ref(),
            body.project_id.trim(),
            body.name.as_deref().unwrap_or("benefit-project-key"),
        )
        .await?,
    ))
}

pub async fn ensure_benefit_project(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Json(body): Json<EnsureBenefitProjectBody>,
) -> Result<Json<db::GatewayBenefitProjectEnsureView>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let pg_pool = state
        .pg_pool
        .as_ref()
        .ok_or_else(|| GatewayError::service_unavailable("PostgreSQL 尚未配置"))?;
    let ensured = db::ensure_benefit_project(
        pg_pool,
        body.service_id.trim(),
        body.user_id.trim(),
        body.service_title.as_deref(),
    )
    .await?;
    let _ = db::ensure_default_access_bundle_for_project(
        pg_pool,
        &state.redis_pool,
        &ensured.project.id,
        &format!("{} 默认访问包", ensured.project.display_name),
    )
    .await?;
    Ok(Json(ensured))
}

pub async fn get_project_api_access(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Path(path): Path<ProjectPath>,
) -> Result<Json<GatewayApiAccessResponse>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    Ok(Json(
        issue_or_get_project_access_key(
            state.as_ref(),
            path.project_id.trim(),
            "benefit-project-key",
        )
        .await?,
    ))
}

pub async fn rotate_project_api_access(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Path(path): Path<ProjectPath>,
    Json(body): Json<RotateProjectApiAccessBody>,
) -> Result<Json<GatewayApiAccessResponse>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let pg_pool = required_pg_pool(state.as_ref())?;
    if let Some(existing) = load_active_project_access_key(pg_pool, path.project_id.trim()).await? {
        let _ = db::rotate_access_key(
            pg_pool,
            &state.redis_pool,
            &existing.id,
            state.config.gateway_api_key_secret.as_deref(),
        )
        .await?;
    }
    Ok(Json(
        issue_or_get_project_access_key(
            state.as_ref(),
            path.project_id.trim(),
            body.name.as_deref().unwrap_or("benefit-project-key"),
        )
        .await?,
    ))
}
