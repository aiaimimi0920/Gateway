//! Qwen freshness and refresh orchestration behind the parent keepalive API.
mod input;
mod runtime;
mod signin;
mod types;
mod worker;

use crate::credential_runtime::SessionAuthConfig;
use crate::error::GatewayError;
use crate::routing::candidate::ProviderAccountPayload;
use deadpool_redis::Pool;
pub(super) use runtime::{apply_qwen_web_runtime_refresh, persist_qwen_web_runtime_refresh};
use signin::execute_qwen_web_http_signin_refresh;
use sqlx::PgPool;
use tracing::warn;
pub(super) use worker::execute_qwen_web_session_worker;

#[cfg(test)]
pub(super) use runtime::merge_qwen_web_runtime_headers;
#[cfg(test)]
pub(super) use signin::{
    qwen_web_signin_headers, qwen_web_signin_password_attempts, qwen_web_signin_seed, sha256_hex,
    QwenWebSigninSeed,
};

const QWEN_WEB_REFRESH_BEFORE_SECS: u64 = 300;
fn qwen_web_effective_session_auth(payload: &ProviderAccountPayload) -> Option<SessionAuthConfig> {
    let mut session_auth = payload.session_auth.clone()?;
    if session_auth.expires_at.is_none() {
        session_auth.expires_at = payload.expires_at.clone();
    }
    Some(session_auth)
}

pub(super) fn qwen_web_should_refresh(
    payload: &ProviderAccountPayload,
    force_refresh: bool,
) -> bool {
    if force_refresh {
        return true;
    }
    if payload.api_key.trim().is_empty() {
        return true;
    }

    qwen_web_effective_session_auth(payload).is_some_and(|session_auth| {
        session_auth.expires_at.is_some()
            && session_auth.expires_within_secs(QWEN_WEB_REFRESH_BEFORE_SECS)
    })
}

pub(super) async fn ensure_qwen_web_payload_ready(
    redis_pool: &Pool,
    pg_pool: Option<&PgPool>,
    payload: &ProviderAccountPayload,
    model: &str,
    force_refresh: bool,
) -> Result<ProviderAccountPayload, GatewayError> {
    if !qwen_web_should_refresh(payload, force_refresh) {
        return Ok(payload.clone());
    }

    let refreshed = match execute_qwen_web_http_signin_refresh(payload).await {
        Ok(Some(refreshed)) => refreshed,
        Ok(None) => execute_qwen_web_session_worker(payload, model).await?,
        Err(error) => {
            warn!(
                provider = "qwen_web_compatible",
                provider_account_id = %payload.base_url,
                error = %error,
                "Qwen Web HTTP signin refresh failed; falling back to browser-backed refresh"
            );
            execute_qwen_web_session_worker(payload, model).await?
        }
    };
    persist_qwen_web_runtime_refresh(redis_pool, pg_pool, payload, &refreshed).await;
    Ok(apply_qwen_web_runtime_refresh(payload, &refreshed))
}

pub async fn refresh_qwen_web_payload_after_challenge(
    redis_pool: &Pool,
    pg_pool: Option<&PgPool>,
    payload: &ProviderAccountPayload,
    model: &str,
) -> Result<ProviderAccountPayload, GatewayError> {
    ensure_qwen_web_payload_ready(redis_pool, pg_pool, payload, model, true).await
}
