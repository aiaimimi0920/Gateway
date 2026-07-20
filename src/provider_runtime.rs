use std::sync::Arc;
use std::time::Duration;

use redis::AsyncCommands;
use rquest::Method;
use tracing::warn;

use crate::db;
use crate::error::{sanitize_provider_error_message, GatewayError};
use crate::protocol::freebuff;
use crate::provider_quota::{self, GatewayProviderQuotaView};
use crate::routing::candidate::ProviderAccountPayload;
use crate::state::AppState;
use crate::upstream::headers::build_upstream_headers;

const CODEX_FIXED_SUPPORTED_MODELS: &[&str] =
    &["gpt-5.4", "gpt-5.4-mini", "gpt-5.3-codex", "gpt-5.2"];

#[derive(Debug, Clone)]
pub struct ProviderProbeOutcome {
    pub ok: bool,
    pub provider_account: db::GatewayProviderAccountView,
    pub error_message: Option<String>,
    pub provider_quota: Option<GatewayProviderQuotaView>,
}

#[derive(Debug, Clone)]
struct ProviderProbeLock {
    key: String,
    token: String,
}

fn provider_error_message_for_persistence(message: &str) -> String {
    sanitize_provider_error_message(message)
}

pub async fn probe_provider_account_for_management(
    state: &Arc<AppState>,
    provider_account_id: &str,
) -> Result<ProviderProbeOutcome, GatewayError> {
    let provider_account_id = provider_account_id.trim();
    let pg_pool = required_pg_pool(state.as_ref())?;
    let provider_account = db::get_provider_account(pg_pool, provider_account_id)
        .await?
        .ok_or_else(|| GatewayError::not_found("Provider account 不存在"))?;
    let payload = crate::routing::candidate::deserialize_provider_payload(
        &provider_account.payload,
        Some(&provider_account.adapter),
    )
    .map_err(|error| {
        GatewayError::server_error(format!(
            "deserialize provider payload for probe {}: {error}",
            provider_account.id
        ))
    })?;
    let lock = acquire_provider_probe_lock(&state.redis_pool, provider_account_id).await?;

    let outcome = match probe_provider_account_payload(state.as_ref(), &provider_account).await {
        Ok(()) => {
            db::clear_provider_runtime_keys(&state.redis_pool, provider_account_id).await?;
            let provider_quota = provider_quota::refresh_provider_quota_snapshot(
                &state.redis_pool,
                state.config.upstream_timeout_secs,
                provider_account_id,
                &payload,
            )
            .await
            .ok()
            .flatten();
            ProviderProbeOutcome {
                ok: true,
                provider_account: db::mark_provider_probe_success(pg_pool, provider_account_id)
                    .await?,
                error_message: None,
                provider_quota,
            }
        }
        Err(error) => {
            let message = provider_error_message_for_persistence(&error.message);
            ProviderProbeOutcome {
                ok: false,
                provider_account: db::mark_provider_probe_failure(
                    pg_pool,
                    provider_account_id,
                    &message,
                )
                .await?,
                error_message: Some(message),
                provider_quota: provider_quota::read_cached_provider_quota_snapshot(
                    &state.redis_pool,
                    provider_account_id,
                )
                .await
                .ok()
                .flatten(),
            }
        }
    };

    let _ = release_provider_probe_lock(&state.redis_pool, &lock).await;
    Ok(outcome)
}

pub async fn sweep_cooling_provider_accounts(
    state: &Arc<AppState>,
    limit: i64,
) -> Result<Vec<ProviderProbeOutcome>, GatewayError> {
    let pg_pool = required_pg_pool(state.as_ref())?;
    let mut outcomes = Vec::new();
    let provider_ids = db::list_expired_cooling_provider_account_ids(pg_pool, limit).await?;

    for provider_account_id in provider_ids {
        let now = now_rfc3339();
        let lock = match acquire_provider_probe_lock(&state.redis_pool, &provider_account_id).await
        {
            Ok(lock) => lock,
            Err(error) if error.code.as_deref() == Some("conflict") => continue,
            Err(error) => return Err(error),
        };

        let fresh = db::get_provider_account(pg_pool, &provider_account_id)
            .await?
            .ok_or_else(|| GatewayError::not_found("Provider account 不存在"))?;
        if fresh.status != "cooling" {
            let _ = release_provider_probe_lock(&state.redis_pool, &lock).await;
            continue;
        }
        if fresh
            .cooldown_until
            .as_ref()
            .is_some_and(|value| value.as_str() > now.as_str())
        {
            let _ = release_provider_probe_lock(&state.redis_pool, &lock).await;
            continue;
        }

        let outcome = match probe_provider_account_payload(state.as_ref(), &fresh).await {
            Ok(()) => {
                db::clear_provider_runtime_keys(&state.redis_pool, &provider_account_id).await?;
                let payload = crate::routing::candidate::deserialize_provider_payload(
                    &fresh.payload,
                    Some(&fresh.adapter),
                )
                .map_err(|error| {
                    GatewayError::server_error(format!(
                        "deserialize provider payload for quota refresh {}: {error}",
                        provider_account_id
                    ))
                })?;
                let provider_quota = provider_quota::refresh_provider_quota_snapshot(
                    &state.redis_pool,
                    state.config.upstream_timeout_secs,
                    &provider_account_id,
                    &payload,
                )
                .await
                .ok()
                .flatten();
                ProviderProbeOutcome {
                    ok: true,
                    provider_account: db::mark_provider_probe_success(
                        pg_pool,
                        &provider_account_id,
                    )
                    .await?,
                    error_message: None,
                    provider_quota,
                }
            }
            Err(error) => {
                let message = provider_error_message_for_persistence(&error.message);
                ProviderProbeOutcome {
                    ok: false,
                    provider_account: db::mark_provider_cooling_retry_failure(
                        pg_pool,
                        &provider_account_id,
                        &message,
                    )
                    .await?,
                    error_message: Some(message),
                    provider_quota: provider_quota::read_cached_provider_quota_snapshot(
                        &state.redis_pool,
                        &provider_account_id,
                    )
                    .await
                    .ok()
                    .flatten(),
                }
            }
        };

        let _ = release_provider_probe_lock(&state.redis_pool, &lock).await;
        outcomes.push(outcome);
    }

    Ok(outcomes)
}

pub async fn sweep_cooling_provider_accounts_best_effort(state: &Arc<AppState>, limit: i64) {
    if let Err(error) = sweep_cooling_provider_accounts(state, limit).await {
        warn!(error = %error, "cooling provider sweep failed");
    }
}

pub async fn record_provider_success(
    state: &Arc<AppState>,
    provider_account_id: &str,
) -> Result<(), GatewayError> {
    if !is_runtime_managed_provider(provider_account_id) {
        return Ok(());
    }
    let Some(pg_pool) = state.pg_pool.as_ref() else {
        return Ok(());
    };
    db::note_provider_runtime_success(&state.redis_pool, pg_pool, provider_account_id).await
}

pub async fn record_provider_candidate_success(
    state: &Arc<AppState>,
    provider_account_id: &str,
    provider_credential_id: Option<&str>,
) -> Result<(), GatewayError> {
    let Some(provider_credential_id) = provider_credential_id else {
        return record_provider_success(state, provider_account_id).await;
    };
    let Some(pg_pool) = state.pg_pool.as_ref() else {
        return Ok(());
    };
    clear_provider_credential_runtime_keys(&state.redis_pool, provider_credential_id).await?;
    db::note_provider_credential_runtime_success(pg_pool, provider_credential_id).await
}

pub async fn record_provider_failure(
    state: &Arc<AppState>,
    provider_account_id: &str,
    route_policy: Option<&db::GatewayRoutePolicyConfig>,
    message: &str,
) -> Result<(), GatewayError> {
    if !is_runtime_managed_provider(provider_account_id) {
        return Ok(());
    }
    let Some(pg_pool) = state.pg_pool.as_ref() else {
        return Ok(());
    };
    let fallback_policy = db::GatewayRoutePolicyConfig::default();
    let message = provider_error_message_for_persistence(message);
    db::note_provider_runtime_failure(
        &state.redis_pool,
        pg_pool,
        provider_account_id,
        route_policy.unwrap_or(&fallback_policy),
        &message,
    )
    .await
}

pub async fn record_provider_candidate_failure(
    state: &Arc<AppState>,
    provider_account_id: &str,
    provider_credential_id: Option<&str>,
    route_policy: Option<&db::GatewayRoutePolicyConfig>,
    message: &str,
) -> Result<(), GatewayError> {
    let Some(provider_credential_id) = provider_credential_id else {
        return record_provider_failure(state, provider_account_id, route_policy, message).await;
    };
    let Some(pg_pool) = state.pg_pool.as_ref() else {
        return Ok(());
    };
    let fallback_policy = db::GatewayRoutePolicyConfig::default();
    let route_policy = route_policy.unwrap_or(&fallback_policy);
    let ttl_seconds = route_policy.circuit_breaker_cooldown_seconds.max(30) as u64;
    let threshold = route_policy.circuit_breaker_threshold.max(1) as u64;
    let permanent_failure = is_permanent_provider_credential_failure(message);
    let message = provider_error_message_for_persistence(message);
    let failure_count_key =
        crate::redis::keys::provider_credential_failure_count_key(provider_credential_id);
    let breaker_open_key =
        crate::redis::keys::provider_credential_breaker_open_key(provider_credential_id);
    let failure_count = {
        let mut conn = state.redis_pool.get().await.map_err(|error| {
            GatewayError::server_error(format!("get redis connection: {error}"))
        })?;
        let failure_count: u64 = conn.incr(&failure_count_key, 1).await.map_err(|error| {
            GatewayError::server_error(format!("increment provider credential failures: {error}"))
        })?;
        let _: bool = conn
            .expire(&failure_count_key, ttl_seconds as i64)
            .await
            .map_err(|error| {
                GatewayError::server_error(format!("expire provider credential failures: {error}"))
            })?;
        if permanent_failure || failure_count >= threshold {
            let _: () = redis::cmd("SET")
                .arg(&breaker_open_key)
                .arg("1")
                .arg("EX")
                .arg(ttl_seconds)
                .query_async(&mut conn)
                .await
                .map_err(|error| {
                    GatewayError::server_error(format!("open provider credential breaker: {error}"))
                })?;
        }
        failure_count
    };

    db::note_provider_credential_runtime_failure(
        pg_pool,
        provider_credential_id,
        failure_count,
        permanent_failure || failure_count >= threshold,
        ttl_seconds,
        &message,
    )
    .await
}

fn is_permanent_provider_credential_failure(message: &str) -> bool {
    if crate::provider_failure::classify_provider_failure(None, None, Some(message)).permanent {
        return true;
    }
    let lower = message.to_ascii_lowercase();
    lower.contains("token_invalidated")
        || lower.contains("token_revoked")
        || lower.contains("token has been invalidated")
        || lower.contains("invalid api key")
        || lower.contains("invalid_api_key")
        || lower.contains("appidnoautherror")
        || lower.contains("deactivated_workspace")
}

pub async fn read_provider_breaker_open(
    redis_pool: &deadpool_redis::Pool,
    provider_account_id: &str,
) -> bool {
    read_runtime_breaker_open(redis_pool, provider_account_id, None).await
}

pub async fn read_runtime_breaker_open(
    redis_pool: &deadpool_redis::Pool,
    provider_account_id: &str,
    provider_credential_id: Option<&str>,
) -> bool {
    let Ok(mut conn) = redis_pool.get().await else {
        return false;
    };
    let key = provider_credential_id
        .map(crate::redis::keys::provider_credential_breaker_open_key)
        .unwrap_or_else(|| crate::redis::keys::provider_breaker_open_key(provider_account_id));
    conn.get::<_, Option<String>>(key)
        .await
        .ok()
        .flatten()
        .is_some()
}

pub async fn probe_provider_account_payload(
    state: &AppState,
    provider_account: &db::GatewayProviderAccountView,
) -> Result<(), GatewayError> {
    let payload = crate::routing::candidate::deserialize_provider_payload(
        &provider_account.payload,
        Some(&provider_account.adapter),
    )
    .map_err(|error| {
        GatewayError::server_error(format!(
            "deserialize provider payload for probe {}: {error}",
            provider_account.id
        ))
    })?;
    let client = rquest::Client::builder()
        .timeout(Duration::from_secs(state.config.upstream_timeout_secs))
        .build()
        .map_err(|error| GatewayError::server_error(format!("build probe http client: {error}")))?;
    let base_url = payload.base_url.trim_end_matches('/').to_string();

    if fixed_models_for_payload(&payload).is_some() {
        return Ok(());
    }

    match payload.canonical_adapter() {
        "openai_compatible" | "anthropic_compatible" => {
            send_probe_request(
                &client,
                Method::GET,
                format!("{base_url}/models"),
                &payload,
                ProbeExpectation::HttpOk,
            )
            .await
        }
        "grok_compatible" => {
            send_probe_request(
                &client,
                Method::GET,
                base_url,
                &payload,
                ProbeExpectation::AllowClientErrors,
            )
            .await
        }
        "freebuff_compatible" => freebuff::probe_payload(&client, &payload).await,
        "search_api_compatible" => {
            let path = payload
                .balance_path
                .clone()
                .or_else(|| payload.search_path.clone())
                .unwrap_or_else(|| "/v1/credits/balance".to_string());
            send_probe_request(
                &client,
                Method::GET,
                build_absolute_url(&base_url, &path),
                &payload,
                ProbeExpectation::HttpOk,
            )
            .await
        }
        "producer_compatible" => {
            send_probe_request(
                &client,
                Method::GET,
                format!("{base_url}/__api/billing/credits"),
                &payload,
                ProbeExpectation::HttpOk,
            )
            .await
        }
        "udio_compatible" => {
            send_probe_request(
                &client,
                Method::GET,
                format!("{base_url}/api/users/current"),
                &payload,
                ProbeExpectation::HttpOk,
            )
            .await
        }
        "custom_http" | "provider_passthrough" => {
            let head_result = send_probe_request(
                &client,
                Method::HEAD,
                base_url.clone(),
                &payload,
                ProbeExpectation::AllowClientErrors,
            )
            .await;
            match head_result {
                Ok(()) => Ok(()),
                Err(_error) => {
                    send_probe_request(
                        &client,
                        Method::GET,
                        base_url,
                        &payload,
                        ProbeExpectation::AllowClientErrors,
                    )
                    .await
                }
            }
        }
        _ => Ok(()),
    }
}

pub fn fixed_models_for_payload(payload: &ProviderAccountPayload) -> Option<Vec<String>> {
    if crate::protocol::chatgpt::official_api::is_chatgpt_codex_backend_payload(payload) {
        return Some(
            CODEX_FIXED_SUPPORTED_MODELS
                .iter()
                .map(|model| (*model).to_string())
                .collect(),
        );
    }
    None
}

fn required_pg_pool(state: &AppState) -> Result<&sqlx::PgPool, GatewayError> {
    state
        .pg_pool
        .as_ref()
        .ok_or_else(|| GatewayError::service_unavailable("PostgreSQL 尚未配置"))
}

fn is_runtime_managed_provider(provider_account_id: &str) -> bool {
    !provider_account_id.starts_with("cred:")
}

pub async fn clear_provider_credential_runtime_keys(
    redis_pool: &deadpool_redis::Pool,
    provider_credential_id: &str,
) -> Result<(), GatewayError> {
    let mut conn = redis_pool
        .get()
        .await
        .map_err(|error| GatewayError::server_error(format!("get redis connection: {error}")))?;
    let _: usize = conn
        .del(&[
            crate::redis::keys::provider_credential_failure_count_key(provider_credential_id),
            crate::redis::keys::provider_credential_breaker_open_key(provider_credential_id),
            crate::redis::keys::provider_credential_quota_snapshot_key(provider_credential_id),
            crate::redis::keys::provider_credential_quota_lock_key(provider_credential_id),
        ])
        .await
        .map_err(|error| {
            GatewayError::server_error(format!("clear provider credential runtime keys: {error}"))
        })?;
    Ok(())
}

async fn acquire_provider_probe_lock(
    redis_pool: &deadpool_redis::Pool,
    provider_account_id: &str,
) -> Result<ProviderProbeLock, GatewayError> {
    let key = crate::redis::keys::provider_probe_lock_key(provider_account_id);
    let token = uuid::Uuid::new_v4().to_string();
    let mut conn = redis_pool
        .get()
        .await
        .map_err(|error| GatewayError::server_error(format!("get redis connection: {error}")))?;
    let acquired: Option<String> = redis::cmd("SET")
        .arg(&key)
        .arg(&token)
        .arg("PX")
        .arg(15_000)
        .arg("NX")
        .query_async(&mut conn)
        .await
        .map_err(|error| {
            GatewayError::server_error(format!("acquire provider probe lock: {error}"))
        })?;

    if acquired.as_deref() != Some("OK") {
        return Err(GatewayError::conflict(
            "当前 provider account 正在执行 probe，请稍后再试。",
        ));
    }

    Ok(ProviderProbeLock { key, token })
}

async fn release_provider_probe_lock(
    redis_pool: &deadpool_redis::Pool,
    lock: &ProviderProbeLock,
) -> Result<(), GatewayError> {
    let mut conn = redis_pool
        .get()
        .await
        .map_err(|error| GatewayError::server_error(format!("get redis connection: {error}")))?;
    let current: Option<String> = conn.get(&lock.key).await.map_err(|error| {
        GatewayError::server_error(format!("read provider probe lock: {error}"))
    })?;
    if current.as_deref() == Some(lock.token.as_str()) {
        let _: usize = conn.del(&lock.key).await.map_err(|error| {
            GatewayError::server_error(format!("release provider probe lock: {error}"))
        })?;
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum ProbeExpectation {
    HttpOk,
    AllowClientErrors,
}

async fn send_probe_request(
    client: &rquest::Client,
    method: Method,
    url: String,
    payload: &ProviderAccountPayload,
    expectation: ProbeExpectation,
) -> Result<(), GatewayError> {
    let response = client
        .request(method, url)
        .headers(build_upstream_headers(payload))
        .send()
        .await
        .map_err(|error| {
            GatewayError::service_unavailable(format!("provider probe request failed: {error}"))
        })?;

    match expectation {
        ProbeExpectation::HttpOk if !response.status().is_success() => Err(GatewayError::conflict(
            format!("Provider probe failed with status {}.", response.status()),
        )),
        ProbeExpectation::AllowClientErrors if response.status().as_u16() >= 500 => {
            Err(GatewayError::conflict(format!(
                "Provider probe failed with status {}.",
                response.status()
            )))
        }
        _ => Ok(()),
    }
}

fn build_absolute_url(base_url: &str, path: &str) -> String {
    if path.starts_with("http://") || path.starts_with("https://") {
        return path.to_string();
    }
    if path.starts_with('/') {
        format!("{base_url}{path}")
    } else {
        format!("{base_url}/{path}")
    }
}

fn now_rfc3339() -> String {
    use time::format_description::well_known::Rfc3339;
    time::OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| "9999-12-31T23:59:59Z".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::routing::candidate::ProviderExecutionMode;
    use std::collections::HashMap;

    fn codex_payload(base_url: &str, originator: Option<&str>) -> ProviderAccountPayload {
        let mut headers = HashMap::new();
        if let Some(originator) = originator {
            headers.insert("Originator".to_string(), originator.to_string());
        }
        ProviderAccountPayload {
            adapter: "openai_compatible".to_string(),
            base_url: base_url.to_string(),
            api_key: "tok".to_string(),
            credential_id: None,
            default_model: Some("gpt-5.4".to_string()),
            headers,
            extra_body: None,
            auth_mode: None,
            anthropic_version: None,
            beta_headers: None,
            auth_header_name: None,
            auth_token: None,
            responses_path: Some("/responses".to_string()),
            chat_completions_path: None,
            completions_path: None,
            embeddings_path: None,
            audio_transcriptions_path: None,
            audio_speech_path: None,
            messages_path: None,
            search_path: None,
            fetch_path: None,
            research_path: None,
            balance_path: None,
            search_query_field: None,
            fetch_urls_field: None,
            session_auth: None,
            keepalive: None,
            execution_mode: Some(ProviderExecutionMode::DirectHttp),
            endpoint_execution_modes: None,
            expires_at: None,
            runtime_state_object_key: None,
            account_name: None,
        }
    }

    #[test]
    fn returns_fixed_models_for_chatgpt_codex_payload() {
        let payload = codex_payload(
            "https://chatgpt.com/backend-api/codex",
            Some("codex_cli_rs"),
        );
        assert_eq!(
            fixed_models_for_payload(&payload),
            Some(vec![
                "gpt-5.4".to_string(),
                "gpt-5.4-mini".to_string(),
                "gpt-5.3-codex".to_string(),
                "gpt-5.2".to_string(),
            ])
        );
    }

    #[test]
    fn does_not_return_fixed_models_for_non_codex_payload() {
        let payload = codex_payload("https://api.openai.com/v1", Some("codex_cli_rs"));
        assert!(fixed_models_for_payload(&payload).is_none());
    }

    #[test]
    fn classifies_deactivated_workspace_as_permanent_credential_failure() {
        assert!(is_permanent_provider_credential_failure(
            r#"{"detail":{"code":"deactivated_workspace"}}"#
        ));
    }

    #[test]
    fn classifies_token_invalidated_as_permanent_credential_failure() {
        assert!(is_permanent_provider_credential_failure(
            "Your authentication token has been invalidated. Please try signing in again."
        ));
    }

    #[test]
    fn classifies_appid_no_auth_error_as_permanent_credential_failure() {
        assert!(is_permanent_provider_credential_failure(
            "AppIdNoAuthError: xop35qwen2b is not authorized for this appid"
        ));
    }

    #[test]
    fn persistence_boundary_redacts_provider_error_secrets() {
        let sanitized = provider_error_message_for_persistence(
            "invalid api key; Authorization: Bearer persisted-secret; session=private-session",
        );

        assert!(sanitized.contains("invalid api key"));
        assert!(sanitized.contains("[REDACTED]"));
        assert!(!sanitized.contains("persisted-secret"));
        assert!(!sanitized.contains("private-session"));
    }
}
