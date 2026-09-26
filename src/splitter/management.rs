use super::{ReloadSplitterRequest, SplitterManager, SplitterWorkerView};
use crate::access_control::{
    authorize_internal_request, unauthenticated_internal_routes_allowed, InternalAccessSurface,
};
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use std::time::Duration;
impl SplitterManager {
    pub(super) fn status_payload(&self) -> serde_json::Value {
        let active_worker_id = self.inner.active_worker_id.read().clone();
        let workers: Vec<SplitterWorkerView> = self
            .inner
            .workers
            .read()
            .values()
            .map(|worker| SplitterWorkerView {
                id: worker.id.clone(),
                port: worker.port,
                base_url: worker.base_url.clone(),
                executable_path: worker.executable_path.clone(),
                status: worker.status.read().clone(),
                started_at: worker.started_at.clone(),
                drain_requested_at: worker.drain_requested_at.read().clone(),
                drain_reason: worker.drain_reason.read().clone(),
                exit_status: worker.exit_status.read().clone(),
                active_requests: worker.active_requests(),
                pid: worker.pid,
            })
            .collect();

        serde_json::json!({
            "splitter": {
                "port": self.inner.config.port,
                "activeWorkerId": active_worker_id,
                "workerCount": workers.len(),
                "workers": workers,
            }
        })
    }
}

pub(super) async fn splitter_healthz() -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "status": "ok",
            "role": "splitter",
        })),
    )
}

pub(super) async fn splitter_readyz(State(manager): State<SplitterManager>) -> impl IntoResponse {
    manager.reconcile_worker_processes().await;
    match manager.active_worker() {
        Some(worker)
            if matches!(
                tokio::time::timeout(
                    splitter_readiness_endpoint_timeout(),
                    manager.worker_is_ready(worker.as_ref()),
                )
                .await,
                Ok(true)
            ) =>
        {
            (
                StatusCode::OK,
                Json(serde_json::json!({
                    "status": "ready",
                    "activeWorkerId": worker.id.clone(),
                    "activeWorkerPort": worker.port,
                })),
            )
        }
        Some(worker) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({
                "status": "not_ready",
                "reason": "active_worker_not_ready",
                "activeWorkerId": worker.id.clone(),
                "activeWorkerPort": worker.port,
            })),
        ),
        None => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({
                "status": "not_ready",
                "reason": "no_active_worker",
            })),
        ),
    }
}

pub(super) fn splitter_readiness_endpoint_timeout() -> Duration {
    std::env::var("GATEWAY_SPLITTER_READINESS_ENDPOINT_TIMEOUT_MS")
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .map(|value| value.clamp(50, 5_000))
        .map(Duration::from_millis)
        .unwrap_or_else(|| Duration::from_millis(1_500))
}

pub(super) async fn splitter_status(
    State(manager): State<SplitterManager>,
    headers: HeaderMap,
) -> impl IntoResponse {
    if let Err(response) = assert_splitter_management_access(&manager, &headers) {
        return response;
    }
    manager.reconcile_worker_processes().await;

    (StatusCode::OK, Json(manager.status_payload())).into_response()
}

pub(super) async fn splitter_reload(
    State(manager): State<SplitterManager>,
    headers: HeaderMap,
    Json(request): Json<ReloadSplitterRequest>,
) -> impl IntoResponse {
    if let Err(response) = assert_splitter_management_access(&manager, &headers) {
        return response;
    }
    manager.reconcile_worker_processes().await;

    match manager.reload_worker(request).await {
        Ok(payload) => (StatusCode::ACCEPTED, Json(payload)).into_response(),
        Err(error) => (
            StatusCode::BAD_GATEWAY,
            Json(serde_json::json!({
                "error": {
                    "message": error.to_string(),
                    "code": "gateway_splitter_reload_failed",
                }
            })),
        )
            .into_response(),
    }
}

fn assert_splitter_management_access(
    manager: &SplitterManager,
    headers: &HeaderMap,
) -> Result<(), Response> {
    authorize_internal_request(
        InternalAccessSurface::Management,
        manager.management_token(),
        unauthenticated_internal_routes_allowed(),
        None,
        headers,
    )
    .map_err(IntoResponse::into_response)
}
