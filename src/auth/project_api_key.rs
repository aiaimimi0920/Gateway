use async_trait::async_trait;
use sqlx::PgPool;

use super::adapter::{AuthAdapter, AuthRequest, AuthResult};
use super::session::AuthenticatedSession;
use crate::db;
use crate::error::GatewayError;
use crate::gateway_api_key::parse_gateway_project_api_key;
use crate::gateway_api_key::verify_gateway_project_api_key;

#[derive(Debug, Clone)]
pub struct ProjectApiKeyAdapter {
    pg_pool: PgPool,
    api_key_secret: String,
}

impl ProjectApiKeyAdapter {
    pub fn new(pg_pool: PgPool, api_key_secret: String) -> Self {
        Self {
            pg_pool,
            api_key_secret,
        }
    }
}

#[async_trait]
impl AuthAdapter for ProjectApiKeyAdapter {
    fn name(&self) -> &str {
        "project_api_key"
    }

    async fn authenticate(&self, request: &AuthRequest) -> Result<AuthResult, GatewayError> {
        let credential = match request.credential() {
            Some(value) if value.starts_with("gw-user-") => {
                return Ok(AuthResult::Rejected {
                    error: "Unsupported project api key type".to_string(),
                    status_code: 401,
                });
            }
            Some(value) => value,
            None => {
                return Ok(AuthResult::Rejected {
                    error: "Missing project api key".to_string(),
                    status_code: 401,
                });
            }
        };

        let Some(parsed) = parse_gateway_project_api_key(credential) else {
            return Ok(AuthResult::Rejected {
                error: "Malformed project api key".to_string(),
                status_code: 401,
            });
        };

        let Some(row) = db::find_access_key_auth_by_id(&self.pg_pool, &parsed.api_key_id).await?
        else {
            return Ok(AuthResult::Rejected {
                error: "Project api key not found".to_string(),
                status_code: 401,
            });
        };

        if let Err(error) = db::validate_access_key_auth(&row) {
            return Ok(AuthResult::Rejected {
                error: error.message,
                status_code: 401,
            });
        }

        if !verify_gateway_project_api_key(
            credential,
            &row.id,
            &row.resolved_project_id,
            &row.resolved_tenant_id,
            &self.api_key_secret,
            &row.public_key_prefix,
        ) {
            return Ok(AuthResult::Rejected {
                error: "Project api key signature mismatch".to_string(),
                status_code: 401,
            });
        }

        let _ = db::touch_access_key_last_used(&self.pg_pool, &row.id).await;

        Ok(AuthResult::Authenticated(AuthenticatedSession {
            project_id: row.resolved_project_id,
            tenant_id: row.resolved_tenant_id,
            user_id: None,
            credential_ref: row.external_key.clone(),
            scopes: db::scope_list_from_metadata(row.metadata.as_ref()),
            api_key_id: row.legacy_gateway_api_key_id.clone(),
            user_credential_id: row.legacy_user_credential_id,
            access_key_id: Some(row.id),
            access_key_kind: Some(row.key_kind),
        }))
    }
}
