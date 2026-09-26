pub mod adapter;
pub mod project_api_key;
pub mod session;
pub mod user_credential;

use tracing::{debug, warn};

use crate::error::GatewayError;
use crate::state::AppState;

use self::adapter::{AuthRequest, AuthResult};
use self::session::{AuthenticatedSession, DEV_MODE_API_KEY_ID, SHARED_SECRET_API_KEY_ID};

/// Authenticate a request against the configured adapter chain, falling back
/// to the shared gateway API key when configured.
pub async fn authenticate_request(
    state: &AppState,
    request: &AuthRequest,
    fallback_credential_ref: Option<String>,
) -> Result<AuthenticatedSession, GatewayError> {
    if state.auth_adapters.is_empty() && state.config.gateway_api_key.is_none() {
        warn!("No auth configured — all requests accepted (dev mode)");
        let (project_id, tenant_id) = resolve_shared_secret_session_scope(state).await;
        return Ok(AuthenticatedSession {
            project_id,
            tenant_id,
            user_id: None,
            credential_ref: fallback_credential_ref,
            scopes: vec!["relay".to_string()],
            api_key_id: Some(DEV_MODE_API_KEY_ID.to_string()),
            user_credential_id: None,
            access_key_id: None,
            access_key_kind: None,
        });
    }

    for adapter in &state.auth_adapters {
        debug!(adapter = adapter.name(), "trying auth adapter");
        match adapter.authenticate(request).await {
            Ok(AuthResult::Authenticated(session)) => {
                return Ok(session);
            }
            Ok(AuthResult::Rejected { .. }) => {
                continue;
            }
            Err(error) => {
                debug!(adapter = adapter.name(), error = %error, "auth adapter returned error");
            }
        }
    }

    if let Some(ref expected_key) = state.config.gateway_api_key {
        let provided = request.credential().unwrap_or("");
        if !provided.is_empty() && provided == expected_key {
            let (project_id, tenant_id) = resolve_shared_secret_session_scope(state).await;
            return Ok(AuthenticatedSession {
                project_id,
                tenant_id,
                user_id: None,
                credential_ref: fallback_credential_ref,
                scopes: vec!["relay".to_string()],
                api_key_id: Some(SHARED_SECRET_API_KEY_ID.to_string()),
                user_credential_id: None,
                access_key_id: None,
                access_key_kind: None,
            });
        }
    }

    Err(GatewayError::unauthorized("Authentication failed"))
}

async fn resolve_shared_secret_session_scope(state: &AppState) -> (String, String) {
    let project_id = state.config.default_project_id.clone();
    if let Some(pool) = state.pg_pool.as_ref() {
        if let Ok(Some(tenant_id)) = sqlx::query_scalar::<_, String>(
            r#"
            select tenant_id
            from gateway_projects
            where id = $1 and status = 'active'
            limit 1
            "#,
        )
        .bind(&project_id)
        .fetch_optional(pool)
        .await
        {
            return (project_id, tenant_id);
        }
    }

    (project_id, "default".to_string())
}
