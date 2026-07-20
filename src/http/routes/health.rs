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

    if !redis_ok || !database_ok {
        return readiness_response(
            StatusCode::SERVICE_UNAVAILABLE,
            serde_json::json!({
                "status": "not_ready",
                "reason": if !redis_ok { "Redis connection failed" } else { "Database connection failed" },
                "redis": redis_ok,
                "database": database_ok,
            }),
        );
    }

    let auth_configured = !state.auth_adapters.is_empty() || state.config.gateway_api_key.is_some();

    readiness_response(
        StatusCode::OK,
        serde_json::json!({
            "status": "ready",
            "redis": true,
            "database": database_ok,
            "auth_configured": auth_configured,
            "active_requests": state.lifecycle.active_requests(),
        }),
    )
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
    use axum::response::IntoResponse;
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
}
