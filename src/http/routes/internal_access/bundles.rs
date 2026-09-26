//! Management access-bundle lifecycle and bundle-scoped user-key issuance.

use super::super::internal_gateway::assert_management_access;
use super::required_pg_pool;
use crate::db;
use crate::error::GatewayError;
use crate::http::extractors::OptionalBearerToken;
use crate::state::AppState;
use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::Json;
use serde::Deserialize;
use sqlx::FromRow;
use std::sync::Arc;

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
pub struct BundlePath {
    pub bundle_id: String,
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
