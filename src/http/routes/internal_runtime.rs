use std::sync::Arc;
use std::time::Duration;

use axum::extract::State;
use axum::http::HeaderMap as AxumHeaderMap;
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use time::OffsetDateTime;
use tokio::time::{timeout, timeout_at, Instant as TokioInstant};

use crate::db;
use crate::error::GatewayError;
use crate::http::extractors::OptionalBearerToken;
use crate::metrics::request::global_gateway_metrics;
use crate::runtime::mark_runtime_draining;
use crate::state::AppState;

use super::internal_gateway::assert_management_access;

#[derive(Debug, Deserialize)]
pub struct DrainGatewayRuntimeRequest {
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DependencyReadiness {
    pub configured: bool,
    pub required: bool,
    pub ready: bool,
    pub timed_out: bool,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ReadinessProbeOutcome {
    pub ready: bool,
    pub timed_out: bool,
}

pub async fn bounded_readiness_probe<F>(deadline: Duration, probe: F) -> ReadinessProbeOutcome
where
    F: std::future::Future<Output = bool>,
{
    match timeout(deadline, probe).await {
        Ok(ready) => ReadinessProbeOutcome {
            ready,
            timed_out: false,
        },
        Err(_) => ReadinessProbeOutcome {
            ready: false,
            timed_out: true,
        },
    }
}

#[derive(Debug, Clone)]
struct GatewayRuntimeReadiness {
    redis: DependencyReadiness,
    postgresql: DependencyReadiness,
    object_storage: DependencyReadiness,
    object_storage_driver: String,
    api_key_secret: bool,
    public_base_url: bool,
    provider_stats: db::GatewayReadinessProviderStatsView,
}

enum ProviderStatsProbeResult {
    Ready(db::GatewayReadinessProviderStatsView),
    Error,
    TimedOut,
    Skipped,
}

async fn bounded_readiness_outcome_at<F>(deadline: TokioInstant, probe: F) -> ReadinessProbeOutcome
where
    F: std::future::Future<Output = ReadinessProbeOutcome>,
{
    match timeout_at(deadline, probe).await {
        Ok(outcome) => outcome,
        Err(_) => ReadinessProbeOutcome {
            ready: false,
            timed_out: true,
        },
    }
}

async fn bounded_runtime_readiness_probes<R, D, O, S>(
    deadline: TokioInstant,
    redis_probe: R,
    database_probe: D,
    object_storage_probe: O,
    provider_stats_probe: S,
) -> (
    ReadinessProbeOutcome,
    ReadinessProbeOutcome,
    ReadinessProbeOutcome,
    ProviderStatsProbeResult,
)
where
    R: std::future::Future<Output = ReadinessProbeOutcome>,
    D: std::future::Future<Output = ReadinessProbeOutcome>,
    O: std::future::Future<Output = ReadinessProbeOutcome>,
    S: std::future::Future<Output = ProviderStatsProbeResult>,
{
    tokio::join!(
        bounded_readiness_outcome_at(deadline, redis_probe),
        bounded_readiness_outcome_at(deadline, database_probe),
        bounded_readiness_outcome_at(deadline, object_storage_probe),
        async {
            match timeout_at(deadline, provider_stats_probe).await {
                Ok(result) => result,
                Err(_) => ProviderStatsProbeResult::TimedOut,
            }
        }
    )
}

impl GatewayRuntimeReadiness {
    fn overall_ready(&self, draining: bool) -> bool {
        // Internal readiness keeps its existing enterprise configuration gates;
        // public /readyz intentionally has a narrower traffic-admission contract.
        self.redis.ready
            && self.postgresql.ready
            && self.object_storage.ready
            && self.api_key_secret
            && self.public_base_url
            && !draining
    }
}

pub fn optional_postgresql_readiness(configured: bool, probe_ready: bool) -> DependencyReadiness {
    DependencyReadiness {
        configured,
        required: configured,
        ready: !configured || probe_ready,
        timed_out: false,
    }
}

pub async fn get_gateway_readiness(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let readiness = probe_gateway_runtime_readiness(state.as_ref()).await;
    let draining = state.lifecycle.is_draining();

    Ok(Json(serde_json::json!({
        "readiness": {
            "ok": readiness.overall_ready(draining),
            "checks": {
                "database": readiness.postgresql.ready,
                "databaseConfigured": readiness.postgresql.configured,
                "redis": readiness.redis.ready,
                "objectStorage": readiness.object_storage.ready,
                "apiKeySecret": readiness.api_key_secret,
                "publicBaseUrl": readiness.public_base_url,
                "draining": !draining,
            },
            "dependencies": {
                "redis": readiness.redis,
                "postgresql": readiness.postgresql,
                "objectStorage": {
                    "configured": readiness.object_storage.configured,
                    "required": readiness.object_storage.required,
                    "ready": readiness.object_storage.ready,
                    "timedOut": readiness.object_storage.timed_out,
                    "driver": readiness.object_storage_driver,
                },
            },
            "draining": draining,
            "drainStartedAt": state.lifecycle.drain_started_at(),
            "drainReason": state.lifecycle.drain_reason(),
            "activeRequests": state.lifecycle.active_requests(),
            "providerStats": readiness.provider_stats,
        }
    })))
}

pub async fn get_gateway_operator_summary(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;

    let readiness = probe_gateway_runtime_readiness(state.as_ref()).await;
    let request_metrics = global_gateway_metrics().snapshot();
    let draining = state.lifecycle.is_draining();
    let published_route_count = state.route_config.list_models().len();
    let generated_at = OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "unknown".to_string());

    Ok(Json(serde_json::json!({
        "summary": {
            "schemaVersion": 1,
            "generatedAt": generated_at,
            "build": {
                "name": env!("CARGO_PKG_NAME"),
                "version": env!("CARGO_PKG_VERSION"),
                "target": {
                    "os": std::env::consts::OS,
                    "arch": std::env::consts::ARCH,
                },
                "debugAssertions": cfg!(debug_assertions),
            },
            "runtime": {
                "role": runtime_role_name(state.config.runtime_role),
                "processId": std::process::id(),
                "port": state.config.port,
            },
            "lifecycle": {
                "state": if draining { "draining" } else { "serving" },
                "draining": draining,
                "activeRequests": state.lifecycle.active_requests(),
                "drainStartedAt": state.lifecycle.drain_started_at(),
                "drainReason": state.lifecycle.drain_reason(),
                "shutdownRequested": state.shutdown.is_requested(),
                "shutdownRequestedAt": state.shutdown.requested_at(),
                "shutdownReason": state.shutdown.reason(),
            },
            "readiness": {
                "ok": readiness.overall_ready(draining),
                "dependencies": {
                    "redis": readiness.redis,
                    "postgresql": readiness.postgresql,
                    "objectStorage": {
                        "configured": readiness.object_storage.configured,
                        "required": readiness.object_storage.required,
                        "ready": readiness.object_storage.ready,
                        "timedOut": readiness.object_storage.timed_out,
                        "driver": readiness.object_storage_driver,
                    },
                },
                "configuration": {
                    "apiKeySecret": readiness.api_key_secret,
                    "publicBaseUrl": readiness.public_base_url,
                },
            },
            "routing": {
                "configured": state.route_config.has_routes(),
                "providerCount": state.route_config.provider_count(),
                "routeCount": published_route_count,
                "publishedModelCount": published_route_count,
            },
            "credentialCache": {
                "entryCount": state.credential_cache.entry_count(),
            },
            "requestMetrics": {
                "requestsTotal": request_metrics.requests_total,
                "requestErrorsTotal": request_metrics.request_errors_total,
                "requestDrainRejectionsTotal": request_metrics.request_drain_rejections_total,
                "requestInFlight": request_metrics.request_in_flight,
                "requestDurationMsCount": request_metrics.request_duration_ms_count,
                "requestDurationMsSum": request_metrics.request_duration_ms_sum,
                "rateLimitChecksTotal": request_metrics.rate_limit_checks_total,
                "rateLimitRejectionsTotal": request_metrics.rate_limit_rejections_total,
                "rateLimitStoreFailuresTotal": request_metrics.rate_limit_store_failures_total,
            },
            "providerStats": readiness.provider_stats,
        }
    })))
}

pub async fn drain_gateway_runtime(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    body: Option<Json<DrainGatewayRuntimeRequest>>,
) -> Result<(StatusCode, Json<Value>), GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;

    let reason = body
        .as_ref()
        .and_then(|payload| payload.reason.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("internal_runtime_drain");

    mark_runtime_draining(&state, reason);
    state.shutdown.request(reason);

    Ok((
        StatusCode::ACCEPTED,
        Json(serde_json::json!({
            "draining": state.lifecycle.is_draining(),
            "activeRequests": state.lifecycle.active_requests(),
            "drainStartedAt": state.lifecycle.drain_started_at(),
            "drainReason": state.lifecycle.drain_reason(),
            "shutdownRequestedAt": state.shutdown.requested_at(),
            "shutdownReason": state.shutdown.reason(),
        })),
    ))
}

async fn probe_gateway_runtime_readiness(state: &AppState) -> GatewayRuntimeReadiness {
    let probe_timeout = readiness_probe_timeout();
    let deadline = TokioInstant::now() + probe_timeout;
    let postgresql_configured = state.pg_pool.is_some();
    let (redis_probe, postgresql_probe, object_storage_probe, provider_stats_probe) =
        bounded_runtime_readiness_probes(
            deadline,
            async {
                let ready = match state.redis_pool.get().await {
                    Ok(mut conn) => redis::cmd("PING")
                        .query_async::<String>(&mut conn)
                        .await
                        .map(|pong| pong.eq_ignore_ascii_case("PONG"))
                        .unwrap_or(false),
                    Err(_) => false,
                };
                ReadinessProbeOutcome {
                    ready,
                    timed_out: false,
                }
            },
            async {
                let ready = match state.pg_pool.as_ref() {
                    Some(pool) => sqlx::query("select 1").execute(pool).await.is_ok(),
                    None => false,
                };
                ReadinessProbeOutcome {
                    ready,
                    timed_out: false,
                }
            },
            async {
                match crate::object_storage::GatewayObjectStorage::from_env() {
                    Ok(storage) => {
                        let remaining = deadline.saturating_duration_since(TokioInstant::now());
                        let outcome = storage.probe_readiness(remaining).await;
                        ReadinessProbeOutcome {
                            ready: outcome.ready,
                            timed_out: outcome.timed_out,
                        }
                    }
                    Err(_) => ReadinessProbeOutcome {
                        ready: false,
                        timed_out: false,
                    },
                }
            },
            async {
                match state.pg_pool.as_ref() {
                    Some(pool) => match db::get_readiness_provider_stats(pool).await {
                        Ok(stats) => ProviderStatsProbeResult::Ready(stats),
                        Err(_) => ProviderStatsProbeResult::Error,
                    },
                    None => ProviderStatsProbeResult::Skipped,
                }
            },
        )
        .await;

    let mut postgresql =
        optional_postgresql_readiness(postgresql_configured, postgresql_probe.ready);
    postgresql.timed_out = postgresql_probe.timed_out;
    let redis = DependencyReadiness {
        configured: true,
        required: true,
        ready: redis_probe.ready,
        timed_out: redis_probe.timed_out,
    };

    let object_storage_driver = object_storage_driver();
    let object_storage = DependencyReadiness {
        configured: true,
        required: true,
        ready: object_storage_probe.ready,
        timed_out: object_storage_probe.timed_out,
    };

    let provider_stats = match postgresql.ready {
        true => apply_provider_stats_probe_result(&mut postgresql, provider_stats_probe),
        _ => db::GatewayReadinessProviderStatsView::default(),
    };

    GatewayRuntimeReadiness {
        redis,
        postgresql,
        object_storage,
        object_storage_driver,
        api_key_secret: state
            .config
            .gateway_api_key_secret
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty()),
        public_base_url: public_base_url_configured(),
        provider_stats,
    }
}

fn apply_provider_stats_probe_result(
    postgresql: &mut DependencyReadiness,
    result: ProviderStatsProbeResult,
) -> db::GatewayReadinessProviderStatsView {
    match result {
        ProviderStatsProbeResult::Ready(stats) => stats,
        ProviderStatsProbeResult::Error => {
            postgresql.ready = false;
            db::GatewayReadinessProviderStatsView::default()
        }
        ProviderStatsProbeResult::TimedOut => {
            postgresql.ready = false;
            postgresql.timed_out = true;
            db::GatewayReadinessProviderStatsView::default()
        }
        ProviderStatsProbeResult::Skipped => db::GatewayReadinessProviderStatsView::default(),
    }
}

pub(crate) fn readiness_probe_timeout() -> Duration {
    std::env::var("GATEWAY_READINESS_PROBE_TIMEOUT_MS")
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .map(|value| value.clamp(50, 30_000))
        .map(Duration::from_millis)
        .unwrap_or_else(|| Duration::from_millis(1_500))
}

fn object_storage_driver() -> String {
    std::env::var("AI_GATEWAY_OBJECT_STORAGE_DRIVER")
        .ok()
        .or_else(|| std::env::var("OBJECT_STORAGE_DRIVER").ok())
        .map(|value| value.trim().to_lowercase())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "local".to_string())
}

fn runtime_role_name(role: crate::config::GatewayRuntimeRole) -> &'static str {
    match role {
        crate::config::GatewayRuntimeRole::Splitter => "splitter",
        crate::config::GatewayRuntimeRole::Worker => "worker",
        crate::config::GatewayRuntimeRole::Standalone => "standalone",
    }
}

fn public_base_url_configured() -> bool {
    required_env(&[
        "GATEWAY_PUBLIC_BASE_URL",
        "AI_GATEWAY_PUBLIC_BASE_URL",
        "PUBLIC_BASE_URL",
    ])
    .is_some()
}

fn required_env(keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        std::env::var(key)
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    fn ready_postgresql() -> DependencyReadiness {
        DependencyReadiness {
            configured: true,
            required: true,
            ready: true,
            timed_out: false,
        }
    }

    #[test]
    fn provider_stats_error_marks_postgresql_not_ready_without_timeout() {
        let mut postgresql = ready_postgresql();
        let stats =
            apply_provider_stats_probe_result(&mut postgresql, ProviderStatsProbeResult::Error);

        assert_eq!(stats.active_providers, 0);
        assert_eq!(stats.cooling_providers, 0);
        assert_eq!(stats.disabled_providers, 0);
        assert!(!postgresql.ready);
        assert!(!postgresql.timed_out);
    }

    #[test]
    fn provider_stats_timeout_marks_postgresql_not_ready_and_preserves_timeout() {
        let mut postgresql = ready_postgresql();
        let stats =
            apply_provider_stats_probe_result(&mut postgresql, ProviderStatsProbeResult::TimedOut);

        assert_eq!(stats.active_providers, 0);
        assert_eq!(stats.cooling_providers, 0);
        assert_eq!(stats.disabled_providers, 0);
        assert!(!postgresql.ready);
        assert!(postgresql.timed_out);
    }

    #[tokio::test]
    async fn runtime_readiness_probes_share_one_total_timeout_budget() {
        let started = Instant::now();
        let deadline = tokio::time::Instant::now() + Duration::from_millis(200);
        let (redis, database, object_storage, provider_stats) = bounded_runtime_readiness_probes(
            deadline,
            async {
                tokio::time::sleep(Duration::from_millis(20)).await;
                ReadinessProbeOutcome {
                    ready: true,
                    timed_out: false,
                }
            },
            async {
                tokio::time::sleep(Duration::from_millis(20)).await;
                ReadinessProbeOutcome {
                    ready: true,
                    timed_out: false,
                }
            },
            async {
                tokio::time::sleep(Duration::from_millis(500)).await;
                ReadinessProbeOutcome {
                    ready: true,
                    timed_out: false,
                }
            },
            async {
                tokio::time::sleep(Duration::from_millis(500)).await;
                ProviderStatsProbeResult::Error
            },
        )
        .await;

        assert!(redis.ready);
        assert!(database.ready);
        assert!(!object_storage.ready);
        assert!(object_storage.timed_out);
        assert!(matches!(provider_stats, ProviderStatsProbeResult::TimedOut));
        assert!(started.elapsed() < Duration::from_millis(320));
    }
}
