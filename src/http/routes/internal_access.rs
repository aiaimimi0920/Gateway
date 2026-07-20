use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::Json;
use serde::Deserialize;
use sqlx::FromRow;

use crate::db;
use crate::error::GatewayError;
use crate::http::extractors::OptionalBearerToken;
use crate::protocol::canonical::EndpointKind;
use crate::state::AppState;

use super::internal_gateway::assert_management_access;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogQuery {
    pub include_tokens: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderCapabilityBody {
    pub provider_account_id: String,
    pub model_code: String,
    pub endpoint_kind: String,
    pub upstream_model: Option<String>,
    pub enabled: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformAccessBody {
    pub provider_capability_id: String,
    pub model_code: String,
    pub endpoint_kind: String,
    pub upstream_model: Option<String>,
    pub platform_tier: String,
    pub status: String,
    pub operator_weight: i32,
    pub routing_priority: i32,
    pub enabled_for_sale: bool,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccessBundleBody {
    pub project_id: Option<String>,
    pub slug: String,
    pub display_name: String,
    pub billing_mode: Option<String>,
    pub status: String,
    pub description: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplaceBundleItemsBody {
    pub platform_access_ids: Vec<String>,
}

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
    #[serde(default)]
    pub bundle_ids: Vec<String>,
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
#[serde(rename_all = "camelCase")]
pub struct CandidatePreviewQuery {
    pub access_key_id: String,
    pub model: String,
    pub endpoint_kind: String,
    pub estimated_tokens: Option<u64>,
    pub explicit_session_key: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AffinityQuery {
    pub access_key_id: String,
    pub model: String,
    pub explicit_session_key: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AccessKeyPath {
    pub access_key_id: String,
}

#[derive(Debug, Deserialize)]
pub struct BundlePath {
    pub bundle_id: String,
}

#[derive(Debug, Deserialize)]
pub struct AccessPath {
    pub access_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnsureBundleUserKeyBody {
    pub user_id: String,
    pub display_name: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, FromRow)]
struct BundleDetailRow {
    id: String,
    project_id: Option<String>,
    display_name: String,
    billing_mode: String,
    status: String,
}

#[derive(Debug, FromRow)]
struct ProjectTenantRow {
    id: String,
    tenant_id: String,
    status: String,
}

#[derive(Debug, FromRow)]
struct ExistingUserBundleKeyRow {
    id: String,
}

fn required_pg_pool(state: &AppState) -> Result<&sqlx::PgPool, GatewayError> {
    state
        .pg_pool
        .as_ref()
        .ok_or_else(|| GatewayError::service_unavailable("PostgreSQL 尚未配置"))
}

fn parse_endpoint_kind(raw: &str) -> Result<EndpointKind, GatewayError> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "chat_completions" => Ok(EndpointKind::ChatCompletions),
        "completions" => Ok(EndpointKind::Completions),
        "embeddings" => Ok(EndpointKind::Embeddings),
        "images_generations" => Ok(EndpointKind::ImagesGenerations),
        "images_edits" => Ok(EndpointKind::ImagesEdits),
        "music_generations" => Ok(EndpointKind::MusicGenerations),
        "videos_generations" => Ok(EndpointKind::VideosGenerations),
        "audio_transcriptions" => Ok(EndpointKind::AudioTranscriptions),
        "audio_speech" => Ok(EndpointKind::AudioSpeech),
        "messages" => Ok(EndpointKind::Messages),
        "responses" => Ok(EndpointKind::Responses),
        "search" => Ok(EndpointKind::Search),
        "fetch" => Ok(EndpointKind::Fetch),
        "research_create" => Ok(EndpointKind::ResearchCreate),
        "research_list" => Ok(EndpointKind::ResearchList),
        "research_get" => Ok(EndpointKind::ResearchGet),
        "credits_balance" => Ok(EndpointKind::CreditsBalance),
        other => Err(GatewayError::bad_request(format!(
            "不支持的 endpointKind: {other}"
        ))),
    }
}

fn build_bundle_scoped_key_prefix(bundle_id: &str, billing_mode: &str) -> String {
    let mode_code = match billing_mode.trim().to_ascii_lowercase().as_str() {
        "time_pass" => "tm",
        "message_prepaid" => "rq",
        _ => "tk",
    };
    format!("nl_{mode_code}_{bundle_id}_")
}

async fn load_bundle_detail(
    pool: &sqlx::PgPool,
    bundle_id: &str,
) -> Result<BundleDetailRow, GatewayError> {
    let row = sqlx::query_as::<_, BundleDetailRow>(
        r#"
        select id, project_id, display_name, billing_mode, status
        from gateway_access_bundles
        where id = $1
        limit 1
        "#,
    )
    .bind(bundle_id)
    .fetch_optional(pool)
    .await
    .map_err(|error| GatewayError::server_error(format!("load access bundle: {error}")))?;
    let Some(row) = row else {
        return Err(GatewayError::not_found("Bundle 不存在"));
    };
    if row.status != "active" {
        return Err(GatewayError::conflict("Bundle 未激活"));
    }
    if row.project_id.is_none() {
        return Err(GatewayError::conflict(
            "Bundle 尚未绑定 project，当前无法自动签发用户 Key",
        ));
    }
    Ok(row)
}

async fn load_project_tenant(
    pool: &sqlx::PgPool,
    project_id: &str,
) -> Result<ProjectTenantRow, GatewayError> {
    let row = sqlx::query_as::<_, ProjectTenantRow>(
        r#"
        select id, tenant_id, status
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
        return Err(GatewayError::not_found("Bundle 绑定的 project 不存在"));
    };
    if row.status != "active" {
        return Err(GatewayError::conflict("Bundle 绑定的 project 未激活"));
    }
    Ok(row)
}

async fn find_existing_user_bundle_key(
    pool: &sqlx::PgPool,
    bundle_id: &str,
    user_id: &str,
    project_id: &str,
    public_key_prefix: &str,
) -> Result<Option<String>, GatewayError> {
    let row = sqlx::query_as::<_, ExistingUserBundleKeyRow>(
        r#"
        select k.id
        from gateway_access_keys k
        inner join gateway_access_key_bundle_bindings b
          on b.access_key_id = k.id
        where k.owner_type = 'user'
          and k.owner_id = $1
          and k.resolved_project_id = $2
          and k.status = 'active'
          and k.public_key_prefix = $3
          and b.bundle_id = $4
        order by k.created_at desc, k.id desc
        limit 1
        "#,
    )
    .bind(user_id)
    .bind(project_id)
    .bind(public_key_prefix)
    .bind(bundle_id)
    .fetch_optional(pool)
    .await
    .map_err(|error| GatewayError::server_error(format!("load user bundle key: {error}")))?;
    Ok(row.map(|value| value.id))
}

pub async fn get_access_catalog(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Query(_query): Query<CatalogQuery>,
) -> Result<Json<db::GatewayAccessCatalogView>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let catalog = db::list_access_catalog(
        required_pg_pool(state.as_ref())?,
        state.config.gateway_api_key_secret.as_deref(),
    )
    .await?;
    Ok(Json(catalog))
}

pub async fn create_provider_capability(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Json(body): Json<ProviderCapabilityBody>,
) -> Result<Json<db::GatewayProviderCapabilityView>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    Ok(Json(
        db::save_provider_capability(
            required_pg_pool(state.as_ref())?,
            &state.redis_pool,
            None,
            db::UpsertProviderCapabilityInput {
                provider_account_id: body.provider_account_id,
                model_code: body.model_code,
                endpoint_kind: body.endpoint_kind,
                upstream_model: body.upstream_model,
                enabled: body.enabled,
            },
        )
        .await?,
    ))
}

pub async fn update_provider_capability(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Path(path): Path<AccessPath>,
    Json(body): Json<ProviderCapabilityBody>,
) -> Result<Json<db::GatewayProviderCapabilityView>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    Ok(Json(
        db::save_provider_capability(
            required_pg_pool(state.as_ref())?,
            &state.redis_pool,
            Some(path.access_id.as_str()),
            db::UpsertProviderCapabilityInput {
                provider_account_id: body.provider_account_id,
                model_code: body.model_code,
                endpoint_kind: body.endpoint_kind,
                upstream_model: body.upstream_model,
                enabled: body.enabled,
            },
        )
        .await?,
    ))
}

pub async fn create_platform_access(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Json(body): Json<PlatformAccessBody>,
) -> Result<Json<db::GatewayPlatformAccessView>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    Ok(Json(
        db::save_platform_access(
            required_pg_pool(state.as_ref())?,
            &state.redis_pool,
            None,
            db::UpsertPlatformAccessInput {
                provider_capability_id: body.provider_capability_id,
                model_code: body.model_code,
                endpoint_kind: body.endpoint_kind,
                upstream_model: body.upstream_model,
                platform_tier: body.platform_tier,
                status: body.status,
                operator_weight: body.operator_weight,
                routing_priority: body.routing_priority,
                enabled_for_sale: body.enabled_for_sale,
                notes: body.notes,
            },
        )
        .await?,
    ))
}

pub async fn update_platform_access(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Path(path): Path<AccessPath>,
    Json(body): Json<PlatformAccessBody>,
) -> Result<Json<db::GatewayPlatformAccessView>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    Ok(Json(
        db::save_platform_access(
            required_pg_pool(state.as_ref())?,
            &state.redis_pool,
            Some(path.access_id.as_str()),
            db::UpsertPlatformAccessInput {
                provider_capability_id: body.provider_capability_id,
                model_code: body.model_code,
                endpoint_kind: body.endpoint_kind,
                upstream_model: body.upstream_model,
                platform_tier: body.platform_tier,
                status: body.status,
                operator_weight: body.operator_weight,
                routing_priority: body.routing_priority,
                enabled_for_sale: body.enabled_for_sale,
                notes: body.notes,
            },
        )
        .await?,
    ))
}

pub async fn create_access_bundle(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Json(body): Json<AccessBundleBody>,
) -> Result<Json<db::GatewayAccessBundleView>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    Ok(Json(
        db::save_access_bundle(
            required_pg_pool(state.as_ref())?,
            &state.redis_pool,
            None,
            db::UpsertAccessBundleInput {
                project_id: body.project_id,
                slug: body.slug,
                display_name: body.display_name,
                billing_mode: body.billing_mode.unwrap_or_else(|| "time_pass".to_string()),
                status: body.status,
                description: body.description,
                metadata: body.metadata,
            },
        )
        .await?,
    ))
}

pub async fn update_access_bundle(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Path(path): Path<BundlePath>,
    Json(body): Json<AccessBundleBody>,
) -> Result<Json<db::GatewayAccessBundleView>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    Ok(Json(
        db::save_access_bundle(
            required_pg_pool(state.as_ref())?,
            &state.redis_pool,
            Some(path.bundle_id.as_str()),
            db::UpsertAccessBundleInput {
                project_id: body.project_id,
                slug: body.slug,
                display_name: body.display_name,
                billing_mode: body.billing_mode.unwrap_or_else(|| "time_pass".to_string()),
                status: body.status,
                description: body.description,
                metadata: body.metadata,
            },
        )
        .await?,
    ))
}

pub async fn delete_access_bundle(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Path(path): Path<BundlePath>,
) -> Result<Json<db::DeleteAccessBundleResult>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    Ok(Json(
        db::delete_access_bundle(
            required_pg_pool(state.as_ref())?,
            &state.redis_pool,
            &path.bundle_id,
        )
        .await?,
    ))
}

pub async fn replace_bundle_items(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Path(path): Path<BundlePath>,
    Json(body): Json<ReplaceBundleItemsBody>,
) -> Result<Json<Vec<db::GatewayAccessBundleItemView>>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    Ok(Json(
        db::replace_access_bundle_items(
            required_pg_pool(state.as_ref())?,
            &state.redis_pool,
            &path.bundle_id,
            &body.platform_access_ids,
        )
        .await?,
    ))
}

pub async fn ensure_bundle_user_key(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Path(path): Path<BundlePath>,
    Json(body): Json<EnsureBundleUserKeyBody>,
) -> Result<Json<db::GatewayAccessKeyView>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let pool = required_pg_pool(state.as_ref())?;
    let bundle = load_bundle_detail(pool, &path.bundle_id).await?;
    let project_id = bundle.project_id.clone().ok_or_else(|| {
        GatewayError::conflict("Bundle 尚未绑定 project，当前无法自动签发用户 Key")
    })?;
    let project = load_project_tenant(pool, &project_id).await?;
    let public_key_prefix = build_bundle_scoped_key_prefix(&bundle.id, &bundle.billing_mode);
    let display_name = body
        .display_name
        .clone()
        .unwrap_or_else(|| format!("{} API Key", bundle.display_name));
    let metadata = match body.metadata {
        Some(mut value) => {
            if let Some(object) = value.as_object_mut() {
                object.insert("bundleId".to_string(), serde_json::json!(bundle.id.clone()));
                object.insert("userId".to_string(), serde_json::json!(body.user_id.trim()));
                object.insert(
                    "source".to_string(),
                    serde_json::json!("bundle_user_key_auto_ensure"),
                );
            }
            Some(value)
        }
        None => Some(serde_json::json!({
            "bundleId": bundle.id.clone(),
            "userId": body.user_id.trim(),
            "source": "bundle_user_key_auto_ensure",
        })),
    };
    let existing_id = find_existing_user_bundle_key(
        pool,
        &bundle.id,
        body.user_id.trim(),
        &project.id,
        &public_key_prefix,
    )
    .await?;

    Ok(Json(
        db::save_access_key(
            pool,
            &state.redis_pool,
            existing_id.as_deref(),
            state.config.gateway_api_key_secret.as_deref(),
            db::UpsertAccessKeyInput {
                owner_type: "user".to_string(),
                owner_id: body.user_id.trim().to_string(),
                resolved_project_id: project.id,
                resolved_tenant_id: project.tenant_id,
                key_kind: "normal".to_string(),
                public_key_prefix,
                display_name,
                expires_at: None,
                metadata,
                bundle_ids: vec![bundle.id],
            },
        )
        .await?,
    ))
}

pub async fn create_access_key(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Json(body): Json<AccessKeyBody>,
) -> Result<Json<db::GatewayAccessKeyView>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    Ok(Json(
        db::save_access_key(
            required_pg_pool(state.as_ref())?,
            &state.redis_pool,
            None,
            state.config.gateway_api_key_secret.as_deref(),
            db::UpsertAccessKeyInput {
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
            },
        )
        .await?,
    ))
}

pub async fn update_access_key(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Path(path): Path<AccessKeyPath>,
    Json(body): Json<AccessKeyBody>,
) -> Result<Json<db::GatewayAccessKeyView>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    Ok(Json(
        db::save_access_key(
            required_pg_pool(state.as_ref())?,
            &state.redis_pool,
            Some(path.access_key_id.as_str()),
            state.config.gateway_api_key_secret.as_deref(),
            db::UpsertAccessKeyInput {
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
            },
        )
        .await?,
    ))
}

pub async fn delete_access_key(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Path(path): Path<AccessKeyPath>,
) -> Result<Json<db::DeleteAccessKeyResult>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    Ok(Json(
        db::delete_access_key(
            required_pg_pool(state.as_ref())?,
            &state.redis_pool,
            &path.access_key_id,
        )
        .await?,
    ))
}

pub async fn rotate_access_key(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Path(path): Path<AccessKeyPath>,
) -> Result<Json<db::GatewayAccessKeyView>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    Ok(Json(
        db::rotate_access_key(
            required_pg_pool(state.as_ref())?,
            &state.redis_pool,
            &path.access_key_id,
            state.config.gateway_api_key_secret.as_deref(),
        )
        .await?,
    ))
}

pub async fn revoke_access_key(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Path(path): Path<AccessKeyPath>,
    Json(body): Json<RevokeAccessKeyBody>,
) -> Result<Json<serde_json::Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    db::revoke_access_key(
        required_pg_pool(state.as_ref())?,
        &state.redis_pool,
        &path.access_key_id,
        body.reason.as_deref(),
    )
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
        db::adjust_access_key_balance(
            required_pg_pool(state.as_ref())?,
            &state.redis_pool,
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
        db::get_access_key_balance(
            required_pg_pool(state.as_ref())?,
            &state.redis_pool,
            &path.access_key_id,
        )
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

pub async fn preview_candidates(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Query(query): Query<CandidatePreviewQuery>,
) -> Result<Json<Vec<db::GatewayAccessCandidatePreviewView>>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    Ok(Json(
        db::preview_access_candidates(
            required_pg_pool(state.as_ref())?,
            &state.redis_pool,
            &query.access_key_id,
            &query.model,
            parse_endpoint_kind(&query.endpoint_kind)?,
            query.estimated_tokens.unwrap_or(1),
            query.explicit_session_key.as_deref(),
        )
        .await?,
    ))
}

pub async fn preview_route_decision(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Query(query): Query<CandidatePreviewQuery>,
) -> Result<Json<db::GatewayAccessRouteDecisionPreviewView>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    Ok(Json(
        db::preview_route_decision(
            required_pg_pool(state.as_ref())?,
            &state.redis_pool,
            &query.access_key_id,
            &query.model,
            parse_endpoint_kind(&query.endpoint_kind)?,
            query.estimated_tokens.unwrap_or(1),
            query.explicit_session_key.as_deref(),
        )
        .await?,
    ))
}

pub async fn inspect_affinity(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Query(query): Query<AffinityQuery>,
) -> Result<Json<serde_json::Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    Ok(Json(serde_json::json!({
        "affinity": db::inspect_access_sticky_affinity(
            &state.redis_pool,
            &query.access_key_id,
            &query.model,
            query.explicit_session_key.as_deref(),
        ).await?
    })))
}

pub async fn reset_affinity(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Json(query): Json<AffinityQuery>,
) -> Result<Json<serde_json::Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    db::reset_access_sticky_affinity(
        &state.redis_pool,
        &query.access_key_id,
        &query.model,
        query.explicit_session_key.as_deref(),
    )
    .await?;
    Ok(Json(serde_json::json!({ "success": true })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_endpoint_kind_supports_legacy_completions() {
        assert!(matches!(
            parse_endpoint_kind("completions"),
            Ok(EndpointKind::Completions)
        ));
    }

    #[test]
    fn parse_endpoint_kind_supports_audio_speech() {
        assert!(matches!(
            parse_endpoint_kind("audio_speech"),
            Ok(EndpointKind::AudioSpeech)
        ));
    }

    #[test]
    fn parse_endpoint_kind_supports_embeddings() {
        assert!(matches!(
            parse_endpoint_kind("embeddings"),
            Ok(EndpointKind::Embeddings)
        ));
    }
}
