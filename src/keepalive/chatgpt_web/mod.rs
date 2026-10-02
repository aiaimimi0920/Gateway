//! ChatGPT keepalive orchestration; the parent retains the multi-provider API.
mod input;
mod oauth;
mod policy;
mod runtime;
mod types;
mod worker;

use super::{read_extra_body_string, request_time_local_browser_worker_blocking_error};
use crate::error::GatewayError;
use crate::protocol::canonical::CanonicalRelayRequest;
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::request_time_browser_policy::RequestTimeBrowserPolicy;
use deadpool_redis::Pool;
use oauth::chatgpt_web_should_oauth_refresh;
use policy::chatgpt_web_request_time_browser_fallback_allowed;
use rquest::Client;
use sqlx::PgPool;
use tracing::warn;
use types::ChatGptWebSessionWorkerRelayRequest;
use worker::execute_chatgpt_web_session_worker_internal;

pub(crate) use oauth::chatgpt_web_oauth_refresh_due;
pub(super) use oauth::execute_chatgpt_web_oauth_refresh;
pub(super) use policy::chatgpt_web_should_refresh;
pub(super) use runtime::{apply_chatgpt_web_runtime_refresh, persist_chatgpt_web_runtime_refresh};
pub(super) use worker::execute_chatgpt_web_session_worker;

#[cfg(test)]
pub(super) use policy::chatgpt_web_request_time_browser_fallback_allowed_for_policy;
#[cfg(test)]
pub(super) use runtime::merge_chatgpt_web_runtime_headers;

#[derive(Debug, Clone)]
pub struct ChatGptWebBrowserRelayResult {
    pub payload: ProviderAccountPayload,
    pub status: u16,
    pub content_type: Option<String>,
    pub body_text: String,
}

pub(super) async fn ensure_chatgpt_web_payload_ready(
    redis_pool: &Pool,
    pg_pool: Option<&PgPool>,
    payload: &ProviderAccountPayload,
    force_refresh: bool,
) -> Result<ProviderAccountPayload, GatewayError> {
    if chatgpt_web_should_oauth_refresh(payload, force_refresh) {
        match execute_chatgpt_web_oauth_refresh(&crate::http_client::client(), payload).await {
            Ok(refreshed) => {
                persist_chatgpt_web_runtime_refresh(redis_pool, pg_pool, payload, &refreshed).await;
                return Ok(apply_chatgpt_web_runtime_refresh(payload, &refreshed));
            }
            Err(error) => {
                if force_refresh || chatgpt_web_should_refresh(payload, force_refresh) {
                    if let Some(policy_error) = request_time_local_browser_worker_blocking_error(
                        RequestTimeBrowserPolicy::from_env(),
                        "chatgpt_web_reverse_compatible",
                    ) {
                        return Err(policy_error);
                    }
                    if !chatgpt_web_request_time_browser_fallback_allowed(payload) {
                        return Err(error);
                    }
                    warn!(
                        error = %error,
                        "ChatGPT Web OAuth refresh failed; falling back to browser materialization"
                    );
                } else {
                    warn!(
                        error = %error,
                        "ChatGPT Web OAuth refresh failed before hard expiry; keeping current payload"
                    );
                    return Ok(payload.clone());
                }
            }
        }
    }
    if !chatgpt_web_should_refresh(payload, force_refresh) {
        return Ok(payload.clone());
    }

    let refreshed = execute_chatgpt_web_session_worker(payload).await?;
    persist_chatgpt_web_runtime_refresh(redis_pool, pg_pool, payload, &refreshed).await;
    Ok(apply_chatgpt_web_runtime_refresh(payload, &refreshed))
}

pub async fn refresh_chatgpt_web_oauth_payload_if_due(
    redis_pool: &Pool,
    pg_pool: Option<&PgPool>,
    http: &Client,
    payload: &ProviderAccountPayload,
    refresh_before_secs: u64,
    force_refresh: bool,
) -> Result<Option<ProviderAccountPayload>, GatewayError> {
    if !chatgpt_web_oauth_refresh_due(payload, refresh_before_secs, force_refresh) {
        return Ok(None);
    }

    let refreshed = execute_chatgpt_web_oauth_refresh(http, payload).await?;
    persist_chatgpt_web_runtime_refresh(redis_pool, pg_pool, payload, &refreshed).await;
    Ok(Some(apply_chatgpt_web_runtime_refresh(payload, &refreshed)))
}

pub async fn refresh_chatgpt_web_payload_after_challenge(
    redis_pool: &Pool,
    pg_pool: Option<&PgPool>,
    payload: &ProviderAccountPayload,
) -> Result<ProviderAccountPayload, GatewayError> {
    ensure_chatgpt_web_payload_ready(redis_pool, pg_pool, payload, true).await
}

pub async fn execute_chatgpt_web_browser_relay(
    redis_pool: &Pool,
    pg_pool: Option<&PgPool>,
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
    stream: bool,
) -> Result<ChatGptWebBrowserRelayResult, GatewayError> {
    let timezone = read_extra_body_string(payload.extra_body.as_ref(), &["timezone"])
        .unwrap_or_else(|| {
            crate::protocol::chatgpt::web_reverse::CHATGPT_WEB_DEFAULT_TIMEZONE.to_string()
        });
    let relay_body = crate::protocol::chatgpt::web_reverse::pack_request(req, model, &timezone)?;
    let execution = execute_chatgpt_web_session_worker_internal(
        payload,
        Some(ChatGptWebSessionWorkerRelayRequest {
            stream,
            body: relay_body,
        }),
    )
    .await?;
    persist_chatgpt_web_runtime_refresh(redis_pool, pg_pool, payload, &execution.refreshed).await;
    let refreshed_payload = apply_chatgpt_web_runtime_refresh(payload, &execution.refreshed);
    let relay = execution.relay_response.ok_or_else(|| {
        GatewayError::service_unavailable(
            "ChatGPT Web browser relay worker completed without returning any relay response.",
        )
        .with_provider("chatgpt_web_reverse_compatible")
        .with_code("chatgpt_web_browser_relay_missing_response")
    })?;
    let status = relay.status.or(relay.requirements_status).unwrap_or(0);
    let content_type = relay
        .content_type
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| {
            relay
                .requirements_content_type
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
        });
    let body_text = relay
        .body_text
        .as_deref()
        .or(relay.body_preview.as_deref())
        .or(relay.requirements_preview.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_default();
    if !(200..300).contains(&status) {
        return Err(
            crate::protocol::chatgpt::web_reverse::classify_chatgpt_web_http_error(
                status,
                content_type.as_deref(),
                &body_text,
            ),
        );
    }
    if body_text.trim().is_empty() {
        return Err(GatewayError::service_unavailable(
            "ChatGPT Web browser relay succeeded without returning any response body.",
        )
        .with_provider("chatgpt_web_reverse_compatible")
        .with_code("chatgpt_web_browser_relay_empty_body"));
    }
    Ok(ChatGptWebBrowserRelayResult {
        payload: refreshed_payload,
        status,
        content_type,
        body_text,
    })
}
