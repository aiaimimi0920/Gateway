// ---------------------------------------------------------------------------
// http/routes/metrics.rs — Prometheus /metrics endpoint
// ---------------------------------------------------------------------------

use std::{future::Future, sync::Arc, time::Duration};

use axum::{extract::State, response::IntoResponse};
use tracing::warn;

use crate::{
    db, error::GatewayError, http::routes::internal_runtime::readiness_probe_timeout,
    metrics::request::global_gateway_metrics, state::AppState,
};

/// GET /metrics
///
/// Returns Prometheus text format metrics for monitoring.
/// Includes: AIMD concurrency state, provider/route stats, credential cache
/// size, and build info.
///
/// No authentication required (same as /healthz).
pub async fn handle_metrics(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let mut output = global_gateway_metrics().render_prometheus();

    // -- AIMD concurrency metrics per provider --------------------------------
    let snapshots = state.concurrency_registry.snapshot_all();
    for (provider_id, snapshot) in &snapshots {
        output.push_str(&format!(
            "gateway_aimd_current_limit{{provider=\"{}\"}} {}\n",
            provider_id, snapshot.current_limit
        ));
        output.push_str(&format!(
            "gateway_aimd_active_count{{provider=\"{}\"}} {}\n",
            provider_id, snapshot.active_count
        ));
        output.push_str(&format!(
            "gateway_aimd_available{{provider=\"{}\"}} {}\n",
            provider_id, snapshot.available
        ));
    }

    // -- Route config stats ---------------------------------------------------
    output.push_str(&format!(
        "gateway_providers_total {}\n",
        state.route_config.provider_count()
    ));
    output.push_str(&format!(
        "gateway_routes_configured {}\n",
        if state.route_config.has_routes() {
            1
        } else {
            0
        }
    ));

    // -- Credential memory cache stats ----------------------------------------
    output.push_str(&format!(
        "gateway_credential_cache_entries {}\n",
        state.credential_cache.entry_count()
    ));

    // -- Prompt cache aggregate metrics --------------------------------------
    if let Some(pg_pool) = state.pg_pool.as_ref() {
        if let Some(metrics) = bounded_prompt_cache_metrics_query(
            readiness_probe_timeout(),
            db::get_prompt_cache_metrics(pg_pool),
        )
        .await
        {
            output.push_str(&format!(
                "gateway_prompt_cache_hit_requests_total {}\n",
                metrics.hit_requests
            ));
            output.push_str(&format!(
                "gateway_prompt_cache_creation_requests_total {}\n",
                metrics.creation_requests
            ));
            output.push_str(&format!(
                "gateway_prompt_cache_client_marked_requests_total {}\n",
                metrics.client_marked_requests
            ));
            output.push_str(&format!(
                "gateway_prompt_cache_auto_applied_requests_total {}\n",
                metrics.auto_applied_requests
            ));
            output.push_str(&format!(
                "gateway_prompt_cache_creation_input_tokens_total {}\n",
                metrics.cache_creation_input_tokens
            ));
            output.push_str(&format!(
                "gateway_prompt_cache_read_input_tokens_total {}\n",
                metrics.cache_read_input_tokens
            ));
        }
    }

    // -- Build info -----------------------------------------------------------
    output.push_str(&format!(
        "gateway_build_info{{version=\"{}\"}} 1\n",
        env!("CARGO_PKG_VERSION")
    ));

    (
        [(
            axum::http::header::CONTENT_TYPE,
            "text/plain; version=0.0.4; charset=utf-8",
        )],
        output,
    )
}

async fn bounded_prompt_cache_metrics_query<F>(
    deadline: Duration,
    query: F,
) -> Option<db::GatewayPromptCacheMetricsView>
where
    F: Future<Output = Result<db::GatewayPromptCacheMetricsView, GatewayError>>,
{
    match tokio::time::timeout(deadline, query).await {
        Ok(Ok(metrics)) => Some(metrics),
        Ok(Err(error)) => {
            warn!(error = %error, "failed to read prompt cache metrics");
            None
        }
        Err(_) => {
            warn!(
                timeout_ms = deadline.as_millis(),
                "prompt cache metrics query timed out"
            );
            None
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use axum::response::IntoResponse;
    use std::time::{Duration, Instant};

    // Verifying the function signature compiles is the main test;
    // integration tests with real AppState require a full server setup.

    #[test]
    fn metrics_module_compiles() {
        // If this compiles, the handler signature and imports are valid.
        fn _assert_handler_signature<F, Fut, R>(_f: F)
        where
            F: FnOnce(State<Arc<AppState>>) -> Fut,
            Fut: std::future::Future<Output = R>,
            R: IntoResponse,
        {
        }
        _assert_handler_signature(handle_metrics);
    }

    #[tokio::test]
    async fn prompt_cache_query_timeout_is_bounded() {
        let started = Instant::now();
        let result = bounded_prompt_cache_metrics_query(
            Duration::from_millis(20),
            std::future::pending::<
                Result<db::GatewayPromptCacheMetricsView, crate::error::GatewayError>,
            >(),
        )
        .await;

        assert!(result.is_none());
        assert!(started.elapsed() < Duration::from_secs(1));
    }
}
