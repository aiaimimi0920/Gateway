use async_trait::async_trait;
use deadpool_redis::Pool;
use sqlx::PgPool;

use super::adapter::{AuthAdapter, AuthRequest, AuthResult};
use super::session::AuthenticatedSession;
use crate::db;
use crate::error::GatewayError;

#[derive(Debug, Clone)]
pub struct UserCredentialAdapter {
    _redis_pool: Pool,
    pg_pool: Option<PgPool>,
}

impl UserCredentialAdapter {
    pub fn new(redis_pool: Pool, pg_pool: Option<PgPool>) -> Self {
        Self {
            _redis_pool: redis_pool,
            pg_pool,
        }
    }
}

#[async_trait]
impl AuthAdapter for UserCredentialAdapter {
    fn name(&self) -> &str {
        "user_credential"
    }

    async fn authenticate(&self, request: &AuthRequest) -> Result<AuthResult, GatewayError> {
        let credential = match request.credential() {
            Some(value) if value.starts_with("gw-user-") => value,
            Some(_) => {
                return Ok(AuthResult::Rejected {
                    error: "Unsupported credential type".to_string(),
                    status_code: 401,
                });
            }
            None => {
                return Ok(AuthResult::Rejected {
                    error: "Missing user credential".to_string(),
                    status_code: 401,
                });
            }
        };

        let Some(pg_pool) = &self.pg_pool else {
            return Err(GatewayError::service_unavailable(
                "Unified user access auth requires PostgreSQL",
            ));
        };

        let Some(access_key) =
            db::find_access_key_auth_by_external_key(pg_pool, credential).await?
        else {
            return Ok(AuthResult::Rejected {
                error: "User credential not found".to_string(),
                status_code: 401,
            });
        };

        if let Err(error) = db::validate_access_key_auth(&access_key) {
            return Ok(AuthResult::Rejected {
                error: error.message,
                status_code: 401,
            });
        }

        let _ = db::touch_access_key_last_used(pg_pool, &access_key.id).await;

        let user_id = if access_key.owner_type == "user" {
            Some(access_key.owner_id.clone())
        } else {
            access_key
                .metadata
                .as_ref()
                .and_then(|value| value.get("userId").or_else(|| value.get("user_id")))
                .and_then(|value| value.as_str())
                .map(str::to_string)
        };

        Ok(AuthResult::Authenticated(AuthenticatedSession {
            project_id: access_key.resolved_project_id.clone(),
            tenant_id: access_key.resolved_tenant_id.clone(),
            user_id,
            credential_ref: access_key.external_key.clone(),
            scopes: db::scope_list_from_metadata(access_key.metadata.as_ref()),
            api_key_id: access_key.legacy_gateway_api_key_id.clone(),
            user_credential_id: access_key
                .legacy_user_credential_id
                .clone()
                .or_else(|| Some(access_key.id.clone())),
            access_key_id: Some(access_key.id),
            access_key_kind: Some(access_key.key_kind),
        }))
    }
}
