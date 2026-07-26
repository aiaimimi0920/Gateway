// ---------------------------------------------------------------------------
// http/routes/health.rs — liveness and readiness probes
// ---------------------------------------------------------------------------

use std::{future::Future, sync::Arc, time::Duration};

use axum::{
    extract::State,
    http::{HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};

use crate::http::routes::internal_runtime::{
    bounded_readiness_probe, readiness_probe_timeout, ReadinessProbeOutcome,
};
use crate::config::GatewayRuntimeRole;
use crate::state::AppState;

/// GET /healthz — liveness probe.
///
/// Always returns 200 OK with version info as long as the process is running.
pub async fn healthz() -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "status": "ok",
            "version": "0.1.0",
        })),
    )
}

/// GET /readyz — readiness probe.
///
/// Checks that the Redis pool can establish a connection and that at least one
/// core dependency path is available. Returns 200 when the gateway is ready to
/// serve traffic, 503 otherwise.
pub async fn readyz(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    if state.lifecycle.is_draining() {
        return readiness_response(
            StatusCode::SERVICE_UNAVAILABLE,
            serde_json::json!({
                "status": "draining",
                "reason": "Gateway instance is draining for a rolling deployment",
                "active_requests": state.lifecycle.active_requests(),
                "drain_started_at": state.lifecycle.drain_started_at(),
                "drain_reason": state.lifecycle.drain_reason(),
            }),
        );
    }

    let probe_timeout = readiness_probe_timeout();
    let (redis_probe, database_probe) = bounded_public_readiness_probes(
        probe_timeout,
        async {
            match state.redis_pool.get().await {
                Ok(mut conn) => redis::cmd("PING")
                    .query_async::<String>(&mut conn)
                    .await
                    .map(|pong| pong.eq_ignore_ascii_case("PONG"))
                    .unwrap_or(false),
                Err(_) => false,
            }
        },
        async {
            match state.pg_pool.as_ref() {
                Some(pool) => sqlx::query("select 1").execute(pool).await.is_ok(),
                None => true,
            }
        },
    )
    .await;
    let redis_ok = redis_probe.ready;
    let database_ok = database_probe.ready;
    let routing_configured = state.route_config.has_routes();
    let redis_required = public_redis_is_required(state.config.runtime_role, routing_configured);
    let auth_configured = !state.auth_adapters.is_empty() || state.config.gateway_api_key.is_some();
    let runtime_role = public_runtime_role_name(state.config.runtime_role);

    if !redis_ok && !redis_required && database_ok {
        return readiness_response(
            StatusCode::OK,
            serde_json::json!({
                "status": "ready",
                "degraded": true,
                "reason": "Redis connection failed; standalone local routes remain available",
                "redis": false,
                "redis_required": false,
                "database": true,
                "routing_configured": routing_configured,
                "runtime_role": runtime_role,
                "auth_configured": auth_configured,
                "active_requests": state.lifecycle.active_requests(),
            }),
        );
    }

    if !redis_ok || !database_ok {
        return readiness_response(
            StatusCode::SERVICE_UNAVAILABLE,
            serde_json::json!({
                "status": "not_ready",
                "reason": if !redis_ok { "Redis connection failed" } else { "Database connection failed" },
                "redis": redis_ok,
                "redis_required": redis_required,
                "database": database_ok,
                "routing_configured": routing_configured,
                "runtime_role": runtime_role,
            }),
        );
    }

    readiness_response(
        StatusCode::OK,
        serde_json::json!({
            "status": "ready",
            "degraded": false,
            "redis": true,
            "redis_required": redis_required,
            "database": database_ok,
            "routing_configured": routing_configured,
            "runtime_role": runtime_role,
            "auth_configured": auth_configured,
            "active_requests": state.lifecycle.active_requests(),
        }),
    )
}

fn public_runtime_role_name(role: GatewayRuntimeRole) -> &'static str {
    match role {
        GatewayRuntimeRole::Splitter => "splitter",
        GatewayRuntimeRole::Worker => "worker",
        GatewayRuntimeRole::Standalone => "standalone",
    }
}

fn public_redis_is_required(role: GatewayRuntimeRole, routing_configured: bool) -> bool {
    !(matches!(role, GatewayRuntimeRole::Standalone) && routing_configured)
}

async fn bounded_health_probe<F>(deadline: Duration, probe: F) -> ReadinessProbeOutcome
where
    F: Future<Output = bool>,
{
    bounded_readiness_probe(deadline, probe).await
}

async fn bounded_public_readiness_probes<R, D>(
    deadline: Duration,
    redis_probe: R,
    database_probe: D,
) -> (ReadinessProbeOutcome, ReadinessProbeOutcome)
where
    R: Future<Output = bool>,
    D: Future<Output = bool>,
{
    tokio::join!(
        bounded_health_probe(deadline, redis_probe),
        bounded_health_probe(deadline, database_probe)
    )
}

fn readiness_response(status: StatusCode, mut payload: serde_json::Value) -> Response {
    let worker_id = std::env::var("GATEWAY_SPLITTER_WORKER_ID")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    if let Some(worker_id) = worker_id.as_deref() {
        payload["worker_id"] = serde_json::Value::String(worker_id.to_string());
    }

    let mut response = (status, Json(payload)).into_response();
    if let Some(worker_id) = worker_id.and_then(|value| HeaderValue::from_str(&value).ok()) {
        response
            .headers_mut()
            .insert("x-gateway-worker-id", worker_id);
    }
    response
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;
    use crate::concurrency::aimd::AimdConfig;
    use crate::concurrency::registry::ConcurrencyRegistry;
    use crate::config::{Config, GatewayRuntimeRole};
    use crate::routing::config::RouteConfigStore;
    use crate::upstream::client::UpstreamClient;
    use axum::response::IntoResponse;
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    #[tokio::test]
    async fn healthz_returns_200_with_json() {
        let response = healthz().await.into_response();
        assert_eq!(response.status(), StatusCode::OK);

        // Verify content-type header indicates JSON.
        let ct = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        assert!(
            ct.contains("application/json"),
            "expected application/json, got: {ct}"
        );
    }

    #[tokio::test]
    async fn readiness_query_timeout_is_bounded() {
        let started = Instant::now();
        let outcome = bounded_health_probe(Duration::from_millis(20), async {
            std::future::pending::<bool>().await
        })
        .await;

        assert!(!outcome.ready);
        assert!(outcome.timed_out);
        assert!(started.elapsed() < Duration::from_secs(1));
    }

    #[tokio::test]
    async fn public_readiness_dependency_probes_share_one_timeout_budget() {
        let started = Instant::now();
        let (redis, database) = bounded_public_readiness_probes(
            Duration::from_millis(200),
            async {
                tokio::time::sleep(Duration::from_millis(60)).await;
                true
            },
            async {
                tokio::time::sleep(Duration::from_millis(60)).await;
                true
            },
        )
        .await;

        assert!(redis.ready);
        assert!(database.ready);
        assert!(started.elapsed() < Duration::from_millis(110));
    }

    fn make_config(runtime_role: GatewayRuntimeRole) -> Config {
        Config {
            console: Default::default(),
            runtime_role,
            port: 4200,
            redis_url: "redis://localhost".to_string(),
            database_url: None,
            upstream_timeout_secs: 30,
            max_request_body_bytes: 1024 * 1024,
            max_body_chat_completions_bytes: 1024 * 1024,
            max_body_completions_bytes: 1024 * 1024,
            max_body_messages_bytes: 1024 * 1024,
            max_body_responses_bytes: 1024 * 1024,
            max_body_embeddings_bytes: 1024 * 1024,
            max_body_audio_transcriptions_bytes: 8 * 1024 * 1024,
            max_body_audio_speech_bytes: 1024 * 1024,
            max_body_search_bytes: 512 * 1024,
            max_body_fetch_bytes: 512 * 1024,
            max_body_research_bytes: 512 * 1024,
            max_body_images_generations_bytes: 1024 * 1024,
            max_body_images_edits_bytes: 1024 * 1024,
            max_body_music_bytes: 1024 * 1024,
            max_body_videos_bytes: 1024 * 1024,
            response_cache_ttl_secs: 300,
            response_cache_max_size_bytes: 512 * 1024,
            quota_pre_deduct_estimate_ratio: 1.2,
            usage_report_batch_size: 100,
            provider_probe_interval_secs: 30,
            log_level: "info".to_string(),
            gateway_api_key: None,
            gateway_api_key_secret: None,
            gateway_management_token: None,
            gateway_keepalive_bearer_token: None,
            default_project_id: "platform-default-project".to_string(),
            gateway_inbound_api_key_header_aliases: Vec::new(),
            provider_credential_folder_sync_enabled: false,
            provider_credential_folder_sync_root_dir: None,
            provider_credential_folder_sync_interval_secs: 30,
            provider_credential_folder_sync_watch_enabled: true,
            provider_credential_folder_sync_watch_debounce_millis: 1500,
            provider_credential_folder_sync_import_enabled: true,
            provider_credential_folder_sync_export_enabled: true,
            provider_credential_folder_sync_delete_missing: false,
            provider_credential_refresh_enabled: true,
            provider_credential_refresh_interval_secs: 3600,
            provider_credential_refresh_before_secs: 86_400,
            provider_credential_refresh_batch_limit: 100,
            provider_credential_refresh_lock_ttl_secs: 300,
            credential_stock_monitor_enabled: true,
            credential_stock_monitor_interval_secs: 60,
            splitter_worker_executable_path: None,
            splitter_initial_worker_port: 4201,
            splitter_ready_timeout_secs: 120,
            splitter_ready_poll_interval_millis: 500,
            splitter_reload_shutdown_timeout_secs: 600,
        }
    }

    fn test_console_auth_runtime() -> Arc<crate::console::ConsoleAuthRuntime> {
        let temp = std::env::temp_dir().join(format!(
            "gateway-health-console-{}",
            uuid::Uuid::new_v4()
        ));
        let console =
            crate::console::ConsoleConfig::from_values(crate::console::ConsoleConfigValues {
                state_dir: Some(temp.clone()),
                routes_file: Some(temp.join("routes.yaml")),
                ..Default::default()
            })
            .unwrap();
        Arc::new(crate::console::ConsoleAuthRuntime::new(&console, None).unwrap())
    }

    fn sample_route_config() -> RouteConfigStore {
        let yaml = r#"
providers:
  - id: openai-default
    preset: openai
    base_url: "https://api.openai.com"
    api_key: "sk-test"
model_routes:
  - pattern: "gpt-*"
    provider_ids: [openai-default]
    priority: 10
"#;
        let temp = std::env::temp_dir().join(format!(
            "gateway-health-routes-{}.yaml",
            uuid::Uuid::new_v4()
        ));
        std::fs::write(&temp, yaml).unwrap();
        let store = RouteConfigStore::load_from_yaml(&temp).unwrap();
        let _ = std::fs::remove_file(&temp);
        store
    }

    fn make_state(
        runtime_role: GatewayRuntimeRole,
        redis_url: &str,
        route_config: RouteConfigStore,
    ) -> Arc<AppState> {
        Arc::new(AppState {
            config: make_config(runtime_role),
            redis_pool: deadpool_redis::Config::from_url(redis_url)
                .create_pool(Some(deadpool_redis::Runtime::Tokio1))
                .expect("redis pool"),
            pg_pool: None,
            upstream_client: UpstreamClient::new(30),
            concurrency_registry: ConcurrencyRegistry::new(AimdConfig::default()),
            auth_adapters: vec![],
            filter_config: None,
            route_config: Arc::new(route_config),
            route_config_runtime: None,
            console_auth: test_console_auth_runtime(),
            credential_cache: crate::credential_store::CredentialMemoryCache::new(30),
            lifecycle: crate::state::GatewayLifecycleState::default(),
            shutdown: crate::state::GatewayShutdownHandle::default(),
            provider_credential_folder_sync:
                crate::state::ProviderCredentialFolderSyncRuntime::new(false),
        })
    }

    async fn response_json(response: Response) -> serde_json::Value {
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("response body");
        serde_json::from_slice(&body).expect("json body")
    }

    #[tokio::test]
    async fn standalone_readyz_accepts_local_routes_when_redis_is_unavailable() {
        let state = make_state(
            GatewayRuntimeRole::Standalone,
            "redis://127.0.0.1:1",
            sample_route_config(),
        );

        let response = readyz(State(state)).await.into_response();
        let status = response.status();
        let payload = response_json(response).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(payload["status"], "ready");
        assert_eq!(payload["degraded"], true);
        assert_eq!(payload["redis"], false);
        assert_eq!(payload["redis_required"], false);
        assert_eq!(payload["routing_configured"], true);
        assert_eq!(payload["runtime_role"], "standalone");
    }

    #[tokio::test]
    async fn worker_readyz_still_requires_redis_even_with_local_routes() {
        let state = make_state(
            GatewayRuntimeRole::Worker,
            "redis://127.0.0.1:1",
            sample_route_config(),
        );

        let response = readyz(State(state)).await.into_response();
        let status = response.status();
        let payload = response_json(response).await;

        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(payload["status"], "not_ready");
        assert_eq!(payload["reason"], "Redis connection failed");
    }

    #[tokio::test]
    async fn standalone_readyz_without_routes_still_requires_redis() {
        let state = make_state(
            GatewayRuntimeRole::Standalone,
            "redis://127.0.0.1:1",
            RouteConfigStore::new(),
        );

        let response = readyz(State(state)).await.into_response();
        let status = response.status();
        let payload = response_json(response).await;

        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(payload["status"], "not_ready");
        assert_eq!(payload["reason"], "Redis connection failed");
    }
}
