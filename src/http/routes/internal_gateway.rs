use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::Json;
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::FromRow;
use time::OffsetDateTime;

use crate::access_control::{
    authorize_internal_request, unauthenticated_internal_routes_allowed, InternalAccessSurface,
};
use crate::db;
use crate::error::GatewayError;
use crate::gateway_api_key::build_gateway_project_api_key;
use crate::http::extractors::OptionalBearerToken;
use crate::redis::keys;
use crate::redis::usage_tracking::dequeue_usage_reports;
use crate::state::AppState;
use crate::usage_aggregation::aggregate_usage_reports_by_user_credential_model;

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

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectPromptCacheQuery {
    pub created_from: Option<String>,
    pub created_to: Option<String>,
    pub input_price_per_million: Option<f64>,
    pub bucket_size: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageAggregateFlushBody {
    pub batch_size: Option<usize>,
    pub bucket_seconds: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct IssueUserCredentialBody {
    #[serde(alias = "userId")]
    pub user_id: String,
    #[serde(default, alias = "projectId")]
    pub project_id: Option<String>,
    #[serde(alias = "credentialType")]
    pub credential_type: String,
    #[serde(alias = "durationDays")]
    pub duration_days: i64,
    pub scope: Vec<String>,
    pub metadata: Option<Value>,
}

#[derive(Debug, Deserialize)]
pub struct VerifyUserCredentialBody {
    #[serde(alias = "credentialKey")]
    pub credential_key: String,
    pub scope: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RevokeUserCredentialBody {
    #[serde(alias = "credentialKey")]
    pub credential_key: String,
    pub reason: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct UserCredentialIssueResponse {
    pub success: bool,
    pub credential: db::IssuedUserCredential,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct SimpleSuccessResponse {
    pub success: bool,
    pub message: String,
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

#[derive(Debug, Clone, FromRow)]
struct ProjectDetailRow {
    id: String,
    tenant_id: String,
    slug: String,
    display_name: String,
    status: String,
    source_kind: String,
    source_key: String,
    default_route_policy_id: Option<String>,
    created_at: OffsetDateTime,
    updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, FromRow)]
struct TenantDetailRow {
    id: String,
    slug: String,
    display_name: String,
    status: String,
    owner_user_id: Option<String>,
    source_kind: String,
    source_key: String,
    created_at: OffsetDateTime,
    updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, FromRow)]
struct AccessKeyCompatRow {
    id: String,
    resolved_project_id: String,
    resolved_tenant_id: String,
    display_name: String,
    status: String,
    rotated_from_access_key_id: Option<String>,
    revoked_at: Option<OffsetDateTime>,
    created_at: OffsetDateTime,
}

fn format_timestamp(value: OffsetDateTime) -> String {
    value
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_default()
}

fn project_view_from_row(row: ProjectDetailRow) -> db::GatewayProjectView {
    db::GatewayProjectView {
        id: row.id,
        tenant_id: row.tenant_id,
        slug: row.slug,
        display_name: row.display_name,
        status: row.status,
        source_kind: row.source_kind,
        source_key: row.source_key,
        default_route_policy_id: row.default_route_policy_id,
        created_at: format_timestamp(row.created_at),
        updated_at: format_timestamp(row.updated_at),
    }
}

fn tenant_view_from_row(row: TenantDetailRow) -> db::GatewayTenantView {
    db::GatewayTenantView {
        id: row.id,
        slug: row.slug,
        display_name: row.display_name,
        status: row.status,
        owner_user_id: row.owner_user_id,
        source_kind: row.source_kind,
        source_key: row.source_key,
        created_at: format_timestamp(row.created_at),
        updated_at: format_timestamp(row.updated_at),
    }
}

async fn load_project_detail(
    pool: &sqlx::PgPool,
    project_id: &str,
) -> Result<ProjectDetailRow, GatewayError> {
    let row = sqlx::query_as::<_, ProjectDetailRow>(
        r#"
        select
          id, tenant_id, slug, display_name, status, source_kind, source_key,
          default_route_policy_id, created_at, updated_at
        from gateway_projects
        where id = $1
        limit 1
        "#,
    )
    .bind(project_id)
    .fetch_optional(pool)
    .await
    .map_err(|error| GatewayError::server_error(format!("load gateway project: {error}")))?;
    let Some(row) = row else {
        return Err(GatewayError::not_found("AI gateway project 不存在"));
    };
    if row.status != "active" {
        return Err(GatewayError::conflict("AI gateway project 未激活"));
    }
    Ok(row)
}

async fn load_tenant_detail(
    pool: &sqlx::PgPool,
    tenant_id: &str,
) -> Result<TenantDetailRow, GatewayError> {
    let row = sqlx::query_as::<_, TenantDetailRow>(
        r#"
        select
          id, slug, display_name, status, owner_user_id, source_kind, source_key, created_at, updated_at
        from gateway_tenants
        where id = $1
        limit 1
        "#,
    )
    .bind(tenant_id)
    .fetch_optional(pool)
    .await
    .map_err(|error| GatewayError::server_error(format!("load gateway tenant: {error}")))?;
    let Some(row) = row else {
        return Err(GatewayError::not_found("AI gateway tenant 不存在"));
    };
    if row.status != "active" {
        return Err(GatewayError::conflict("AI gateway tenant 未激活"));
    }
    Ok(row)
}

async fn load_active_project_access_key(
    pool: &sqlx::PgPool,
    project_id: &str,
) -> Result<Option<AccessKeyCompatRow>, GatewayError> {
    sqlx::query_as::<_, AccessKeyCompatRow>(
        r#"
        select
          id,
          resolved_project_id,
          resolved_tenant_id,
          display_name,
          status,
          rotated_from_access_key_id,
          revoked_at,
          created_at
        from gateway_access_keys
        where owner_type = 'project'
          and owner_id = $1
          and key_kind = 'normal'
          and public_key_prefix = 'new_api'
          and status = 'active'
        order by created_at desc, id desc
        limit 1
        "#,
    )
    .bind(project_id)
    .fetch_optional(pool)
    .await
    .map_err(|error| GatewayError::server_error(format!("load project access key: {error}")))
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

pub async fn summarize_project_prompt_cache(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Path(path): Path<ProjectPath>,
    Query(query): Query<ProjectPromptCacheQuery>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let summary = db::summarize_prompt_cache(
        required_pg_pool(state.as_ref())?,
        &db::RequestAuditFilters {
            project_id: Some(path.project_id.trim().to_string()),
            created_from: query.created_from,
            created_to: query.created_to,
            limit: query.limit,
            ..db::RequestAuditFilters::default()
        },
        query.input_price_per_million,
    )
    .await?;
    Ok(Json(serde_json::json!({ "summary": summary })))
}

pub async fn get_project_prompt_cache_trend_report(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Path(path): Path<ProjectPath>,
    Query(query): Query<ProjectPromptCacheQuery>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let report = db::get_prompt_cache_trend_report(
        required_pg_pool(state.as_ref())?,
        &db::RequestAuditFilters {
            project_id: Some(path.project_id.trim().to_string()),
            created_from: query.created_from,
            created_to: query.created_to,
            limit: query.limit,
            ..db::RequestAuditFilters::default()
        },
        query.input_price_per_million,
        query.bucket_size.as_deref(),
    )
    .await?;
    Ok(Json(serde_json::json!({ "report": report })))
}

pub async fn list_provider_credential_model_states(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Query(query): Query<db::CredentialModelStateFilters>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let states =
        db::list_provider_credential_model_states(required_pg_pool(state.as_ref())?, query).await?;
    Ok(Json(serde_json::json!({ "states": states })))
}

pub async fn flush_usage_aggregates(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Json(body): Json<UsageAggregateFlushBody>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let batch_size = body
        .batch_size
        .unwrap_or(state.config.usage_report_batch_size)
        .clamp(1, 10_000);
    let bucket_seconds = body.bucket_seconds.unwrap_or(3600).max(60);
    let reports = dequeue_usage_reports(&state.redis_pool, batch_size)
        .await
        .map_err(|error| GatewayError::server_error(format!("dequeue usage reports: {error}")))?;
    let buckets = aggregate_usage_reports_by_user_credential_model(&reports, bucket_seconds);
    db::upsert_usage_aggregate_buckets(required_pg_pool(state.as_ref())?, &buckets).await?;
    Ok(Json(serde_json::json!({
        "dequeued": reports.len(),
        "bucketCount": buckets.len(),
        "buckets": buckets
    })))
}

pub async fn list_usage_aggregates(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Query(query): Query<db::UsageAggregateFilters>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let buckets =
        db::list_usage_aggregate_buckets(required_pg_pool(state.as_ref())?, query).await?;
    Ok(Json(serde_json::json!({ "buckets": buckets })))
}

pub async fn summarize_usage_aggregates(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let queue_depth = usage_queue_depth(state.as_ref()).await.unwrap_or(-1);
    let summary =
        db::summarize_usage_aggregates(required_pg_pool(state.as_ref())?, queue_depth).await?;
    Ok(Json(serde_json::json!({ "summary": summary })))
}

pub async fn issue_user_credential(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Json(body): Json<IssueUserCredentialBody>,
) -> Result<Json<UserCredentialIssueResponse>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    if body.scope.is_empty() {
        return Err(GatewayError::bad_request("scope 不能为空"));
    }
    let pg_pool = required_pg_pool(state.as_ref())?;
    let project_id = body
        .project_id
        .clone()
        .unwrap_or_else(|| state.config.default_project_id.clone());
    let project = load_project_detail(pg_pool, project_id.trim()).await?;
    let tenant = load_tenant_detail(pg_pool, &project.tenant_id).await?;
    let _ = db::ensure_default_access_bundle_for_project(
        pg_pool,
        &state.redis_pool,
        &project.id,
        &format!("{} 默认访问包", project.display_name),
    )
    .await?;
    let expires_at = OffsetDateTime::now_utc() + time::Duration::days(body.duration_days.max(1));
    let mut metadata = body
        .metadata
        .clone()
        .unwrap_or_else(|| serde_json::json!({}));
    if let Some(object) = metadata.as_object_mut() {
        object.insert("scope".to_string(), serde_json::json!(body.scope.clone()));
        object.insert(
            "credentialType".to_string(),
            serde_json::json!(body.credential_type.trim()),
        );
    }
    let access_key = db::save_access_key(
        pg_pool,
        &state.redis_pool,
        None,
        state.config.gateway_api_key_secret.as_deref(),
        db::UpsertAccessKeyInput {
            owner_type: "user".to_string(),
            owner_id: body.user_id.trim().to_string(),
            resolved_project_id: project.id.clone(),
            resolved_tenant_id: tenant.id.clone(),
            key_kind: "normal".to_string(),
            public_key_prefix: "gw-user".to_string(),
            display_name: format!("{} 凭证", body.credential_type.trim()),
            expires_at: Some(format_timestamp(expires_at)),
            metadata: Some(metadata),
            bundle_ids: Vec::new(),
        },
    )
    .await?;
    let issued = db::IssuedUserCredential {
        id: access_key.id.clone(),
        credential_key: access_key
            .token
            .clone()
            .ok_or_else(|| GatewayError::server_error("用户 access key 未生成 token"))?,
        expires_at: access_key
            .expires_at
            .clone()
            .unwrap_or_else(|| format_timestamp(expires_at)),
        scope: body.scope.clone(),
        user_id: body.user_id.trim().to_string(),
        project_id: project.id.clone(),
        tenant_id: tenant.id.clone(),
        credential_type: body.credential_type.trim().to_string(),
    };

    set_cached_user_credential(state.as_ref(), &issued).await?;

    Ok(Json(UserCredentialIssueResponse {
        success: true,
        credential: issued,
        message: "凭证已颁发".to_string(),
    }))
}

pub async fn verify_user_credential(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Json(body): Json<VerifyUserCredentialBody>,
) -> Result<Json<db::VerifiedUserCredential>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let pg_pool = required_pg_pool(state.as_ref())?;
    let verified = if let Some(access_key) =
        db::find_access_key_auth_by_external_key(pg_pool, body.credential_key.trim()).await?
    {
        match db::validate_access_key_auth(&access_key) {
            Ok(()) => {
                let scopes = db::scope_list_from_metadata(access_key.metadata.as_ref());
                if let Some(required_scope) = body.scope.as_deref() {
                    if !scopes.iter().any(|scope| scope == required_scope) {
                        db::VerifiedUserCredential {
                            valid: false,
                            credential: None,
                            reason: Some("缺少所需 scope".to_string()),
                        }
                    } else {
                        db::VerifiedUserCredential {
                            valid: true,
                            credential: Some(db::GatewayUserCredentialCacheEntry {
                                id: access_key.id.clone(),
                                user_id: access_key.owner_id.clone(),
                                project_id: access_key.resolved_project_id.clone(),
                                scope: scopes,
                                expires_at: access_key.expires_at.clone().unwrap_or_default(),
                                status: access_key.status.clone(),
                                tenant_id: access_key.resolved_tenant_id.clone(),
                            }),
                            reason: None,
                        }
                    }
                } else {
                    db::VerifiedUserCredential {
                        valid: true,
                        credential: Some(db::GatewayUserCredentialCacheEntry {
                            id: access_key.id.clone(),
                            user_id: access_key.owner_id.clone(),
                            project_id: access_key.resolved_project_id.clone(),
                            scope: scopes,
                            expires_at: access_key.expires_at.clone().unwrap_or_default(),
                            status: access_key.status.clone(),
                            tenant_id: access_key.resolved_tenant_id.clone(),
                        }),
                        reason: None,
                    }
                }
            }
            Err(error) => db::VerifiedUserCredential {
                valid: false,
                credential: None,
                reason: Some(error.message),
            },
        }
    } else {
        db::VerifiedUserCredential {
            valid: false,
            credential: None,
            reason: Some("凭证不存在".to_string()),
        }
    };
    if let Some(credential) = &verified.credential {
        set_cached_user_credential_from_cache_entry(
            state.as_ref(),
            body.credential_key.trim(),
            credential,
        )
        .await?;
    }
    Ok(Json(verified))
}

pub async fn revoke_user_credential(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Json(body): Json<RevokeUserCredentialBody>,
) -> Result<Json<SimpleSuccessResponse>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let pg_pool = required_pg_pool(state.as_ref())?;
    let Some(access_key) =
        db::find_access_key_auth_by_external_key(pg_pool, body.credential_key.trim()).await?
    else {
        return Err(GatewayError::not_found("凭证不存在"));
    };
    db::revoke_access_key(
        pg_pool,
        &state.redis_pool,
        &access_key.id,
        body.reason.as_deref(),
    )
    .await?;
    delete_cached_user_credential(state.as_ref(), body.credential_key.trim()).await?;

    Ok(Json(SimpleSuccessResponse {
        success: true,
        message: "凭证已撤销".to_string(),
    }))
}

pub(crate) fn assert_management_access(
    state: &AppState,
    bearer_token: Option<&str>,
    headers: &HeaderMap,
) -> Result<(), GatewayError> {
    assert_management_access_with_expected(
        state.config.gateway_management_token.as_deref(),
        unauthenticated_internal_routes_allowed(),
        bearer_token,
        headers,
    )
}

fn assert_management_access_with_expected(
    expected: Option<&str>,
    allow_unauthenticated: bool,
    bearer_token: Option<&str>,
    headers: &HeaderMap,
) -> Result<(), GatewayError> {
    authorize_internal_request(
        InternalAccessSurface::Management,
        expected,
        allow_unauthenticated,
        bearer_token,
        headers,
    )
}

async fn set_cached_user_credential(
    state: &AppState,
    issued: &db::IssuedUserCredential,
) -> Result<(), GatewayError> {
    let payload = serde_json::json!({
        "id": issued.id,
        "userId": issued.user_id,
        "projectId": issued.project_id,
        "scope": issued.scope,
        "expiresAt": issued.expires_at,
        "status": "active",
        "tenantId": issued.tenant_id,
    });
    write_user_credential_cache(
        &state.redis_pool,
        &issued.credential_key,
        &payload,
        &issued.expires_at,
    )
    .await
}

async fn set_cached_user_credential_from_cache_entry(
    state: &AppState,
    credential_key: &str,
    credential: &db::GatewayUserCredentialCacheEntry,
) -> Result<(), GatewayError> {
    let payload = serde_json::json!({
        "id": credential.id,
        "userId": credential.user_id,
        "projectId": credential.project_id,
        "scope": credential.scope,
        "expiresAt": credential.expires_at,
        "status": credential.status,
        "tenantId": credential.tenant_id,
    });
    write_user_credential_cache(
        &state.redis_pool,
        credential_key,
        &payload,
        &credential.expires_at,
    )
    .await
}

async fn write_user_credential_cache(
    redis_pool: &deadpool_redis::Pool,
    credential_key: &str,
    payload: &Value,
    expires_at: &str,
) -> Result<(), GatewayError> {
    let ttl = compute_cache_ttl_secs(expires_at)
        .unwrap_or(300)
        .clamp(60, 300);
    let mut conn = redis_pool
        .get()
        .await
        .map_err(|error| GatewayError::server_error(format!("get redis connection: {error}")))?;
    let serialized = serde_json::to_string(payload).map_err(|error| {
        GatewayError::server_error(format!("serialize user credential cache: {error}"))
    })?;
    conn.set_ex::<_, _, ()>(keys::user_credential_key(credential_key), serialized, ttl)
        .await
        .map_err(|error| {
            GatewayError::server_error(format!("write user credential cache: {error}"))
        })?;
    Ok(())
}

async fn delete_cached_user_credential(
    state: &AppState,
    credential_key: &str,
) -> Result<(), GatewayError> {
    let mut conn =
        state.redis_pool.get().await.map_err(|error| {
            GatewayError::server_error(format!("get redis connection: {error}"))
        })?;
    conn.del::<_, ()>(keys::user_credential_key(credential_key))
        .await
        .map_err(|error| {
            GatewayError::server_error(format!("delete user credential cache: {error}"))
        })?;
    Ok(())
}

async fn usage_queue_depth(state: &AppState) -> Result<i64, GatewayError> {
    let mut conn =
        state.redis_pool.get().await.map_err(|error| {
            GatewayError::server_error(format!("get redis connection: {error}"))
        })?;
    conn.llen::<_, i64>(keys::usage_reports_key())
        .await
        .map_err(|error| GatewayError::server_error(format!("LLEN usage report queue: {error}")))
}

fn compute_cache_ttl_secs(expires_at: &str) -> Option<u64> {
    let parsed =
        time::OffsetDateTime::parse(expires_at, &time::format_description::well_known::Rfc3339)
            .ok()?;
    if parsed <= time::OffsetDateTime::now_utc() {
        return None;
    }
    Some(
        (parsed - time::OffsetDateTime::now_utc())
            .whole_seconds()
            .max(1) as u64,
    )
}

fn required_pg_pool(state: &AppState) -> Result<&sqlx::PgPool, GatewayError> {
    state
        .pg_pool
        .as_ref()
        .ok_or_else(|| GatewayError::service_unavailable("PostgreSQL 尚未配置"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header_map(name: &'static str, value: &'static str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(name, value.parse().expect("valid header value"));
        headers
    }

    #[test]
    fn management_access_rejects_missing_config_by_default() {
        let err = assert_management_access_with_expected(None, false, None, &HeaderMap::new())
            .expect_err("missing management token must fail closed");

        assert_eq!(err.http_status, Some(503));
        assert_eq!(
            err.code.as_deref(),
            Some("gateway_management_token_not_configured")
        );
    }

    #[test]
    fn management_access_allows_missing_config_only_with_explicit_override() {
        assert!(
            assert_management_access_with_expected(None, true, None, &HeaderMap::new()).is_ok()
        );
    }

    #[test]
    fn management_access_accepts_configured_management_headers() {
        assert!(assert_management_access_with_expected(
            Some("secret"),
            false,
            None,
            &header_map("x-management-token", "secret")
        )
        .is_ok());

        assert!(assert_management_access_with_expected(
            Some("secret"),
            false,
            None,
            &header_map("x-internal-api-key", "secret")
        )
        .is_ok());
    }

    #[test]
    fn management_access_accepts_bearer_and_rejects_wrong_token() {
        assert!(assert_management_access_with_expected(
            Some("secret"),
            false,
            Some("secret"),
            &HeaderMap::new()
        )
        .is_ok());

        let err = assert_management_access_with_expected(
            Some("secret"),
            false,
            Some("wrong"),
            &HeaderMap::new(),
        )
        .expect_err("wrong management token must be rejected");
        assert_eq!(err.http_status, Some(401));
    }
}
