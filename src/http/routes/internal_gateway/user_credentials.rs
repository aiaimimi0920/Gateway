//! Management user-credential issuance, verification and revocation.

use super::project_records::{format_timestamp, load_project_detail, load_tenant_detail};
use super::user_credential_cache::{
    delete_cached_user_credential, set_cached_user_credential,
    set_cached_user_credential_from_cache_entry,
};
use super::{assert_management_access, required_pg_pool};
use crate::db;
use crate::error::GatewayError;
use crate::http::extractors::OptionalBearerToken;
use crate::state::AppState;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use time::OffsetDateTime;

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
