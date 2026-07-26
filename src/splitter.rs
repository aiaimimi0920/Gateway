use std::collections::HashMap;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Context};
use axum::body::{to_bytes, Body};
use axum::extract::{
    ws::{Message as AxumWsMessage, WebSocket, WebSocketUpgrade},
    OriginalUri, Request, State,
};
use axum::http::header::{
    CONNECTION, HOST, PROXY_AUTHENTICATE, PROXY_AUTHORIZATION, TE, TRAILER, TRANSFER_ENCODING,
    UPGRADE,
};
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode, Uri};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{any, get, post};
use axum::{Json, Router};
use futures::{SinkExt, StreamExt};
use parking_lot::RwLock;
use rquest::Client;
use rquest_util::Emulation;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use tokio::process::{Child, Command};
use tokio::sync::{Mutex, Notify};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::protocol::Message as TungsteniteMessage;
use uuid::Uuid;

use crate::access_control::{
    authorize_internal_request, unauthenticated_internal_routes_allowed, InternalAccessSurface,
};
use crate::config::Config;

const SPLITTER_STATUS_PATH: &str = "/v1/internal/gateway/splitter/status";
const SPLITTER_RELOAD_PATH: &str = "/v1/internal/gateway/splitter/reload";

#[derive(Clone)]
pub struct SplitterManager {
    inner: Arc<SplitterInner>,
}

struct SplitterInner {
    config: Config,
    ready_client: Client,
    proxy_client: Client,
    workers: RwLock<HashMap<String, Arc<ManagedWorker>>>,
    active_worker_id: RwLock<Option<String>>,
    next_worker_port: AtomicUsize,
    reload_lock: Mutex<()>,
    supervisor_shutdown: AtomicBool,
}

struct ManagedWorker {
    id: String,
    port: u16,
    base_url: String,
    executable_path: String,
    started_at: String,
    status: RwLock<SplitterWorkerStatus>,
    drain_requested_at: RwLock<Option<String>>,
    drain_reason: RwLock<Option<String>>,
    exit_status: RwLock<Option<String>>,
    child: Mutex<Option<Child>>,
    active_requests: AtomicUsize,
    request_notify: Notify,
    pid: Option<u32>,
}

struct WorkerRequestLease {
    worker: Arc<ManagedWorker>,
}

impl Drop for WorkerRequestLease {
    fn drop(&mut self) {
        let previous = self.worker.active_requests.fetch_sub(1, Ordering::SeqCst);
        debug_assert!(previous > 0, "splitter worker request lease underflow");
        if previous == 1 {
            self.worker.request_notify.notify_waiters();
        }
    }
}

impl ManagedWorker {
    fn transition_to(&self, next: SplitterWorkerStatus) -> anyhow::Result<()> {
        let mut status = self.status.write();
        let current = *status;
        *status = current.transition_to(next).map_err(|error| {
            anyhow!(
                "{} for worker {}: {:?} -> {:?}",
                error,
                self.id,
                current,
                next
            )
        })?;
        Ok(())
    }

    fn try_acquire_lease(self: &Arc<Self>) -> Option<WorkerRequestLease> {
        let status = self.status.read();
        if !status.can_receive_traffic() {
            return None;
        }
        self.active_requests.fetch_add(1, Ordering::SeqCst);
        drop(status);
        Some(WorkerRequestLease {
            worker: Arc::clone(self),
        })
    }

    fn active_requests(&self) -> usize {
        self.active_requests.load(Ordering::SeqCst)
    }

    #[cfg(test)]
    async fn wait_for_no_active_requests(&self, timeout: Duration) -> bool {
        self.wait_for_no_active_requests_until(tokio::time::Instant::now() + timeout)
            .await
    }

    async fn wait_for_no_active_requests_until(&self, deadline: tokio::time::Instant) -> bool {
        loop {
            if self.active_requests() == 0 {
                return true;
            }

            let notified = self.request_notify.notified();
            if self.active_requests() == 0 {
                continue;
            }
            if tokio::time::timeout_at(deadline, notified).await.is_err() {
                return self.active_requests() == 0;
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SplitterWorkerStatus {
    Starting,
    Ready,
    Active,
    Draining,
    Exited,
}

impl SplitterWorkerStatus {
    pub fn can_receive_traffic(self) -> bool {
        self == Self::Active
    }

    pub fn transition_to(self, next: Self) -> Result<Self, &'static str> {
        if self == next {
            return Ok(self);
        }

        match (self, next) {
            (Self::Starting, Self::Ready | Self::Draining | Self::Exited)
            | (Self::Ready, Self::Active | Self::Draining | Self::Exited)
            | (Self::Active, Self::Draining | Self::Exited)
            | (Self::Draining, Self::Exited) => Ok(next),
            _ => Err("invalid splitter worker state transition"),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReloadSplitterRequest {
    executable_path: Option<String>,
    worker_port: Option<u16>,
    ready_timeout_secs: Option<u64>,
    shutdown_timeout_secs: Option<u64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SplitterWorkerView {
    id: String,
    port: u16,
    base_url: String,
    executable_path: String,
    status: SplitterWorkerStatus,
    started_at: String,
    drain_requested_at: Option<String>,
    drain_reason: Option<String>,
    exit_status: Option<String>,
    active_requests: usize,
    pid: Option<u32>,
}

impl SplitterManager {
    pub fn new(config: Config) -> anyhow::Result<Self> {
        let ready_client = Client::builder()
            .emulation(Emulation::Chrome131)
            .timeout(Duration::from_secs(
                config.splitter_ready_timeout_secs.max(10),
            ))
            .build()
            .context("failed to build splitter readiness HTTP client")?;
        let proxy_client = Client::builder()
            .emulation(Emulation::Chrome131)
            .build()
            .context("failed to build splitter proxy HTTP client")?;

        Ok(Self {
            inner: Arc::new(SplitterInner {
                next_worker_port: AtomicUsize::new(config.splitter_initial_worker_port as usize),
                config,
                ready_client,
                proxy_client,
                workers: RwLock::new(HashMap::new()),
                active_worker_id: RwLock::new(None),
                reload_lock: Mutex::new(()),
                supervisor_shutdown: AtomicBool::new(false),
            }),
        })
    }

    pub async fn run(self) -> anyhow::Result<()> {
        let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", self.inner.config.port))
            .await
            .context("failed to bind splitter port")?;
        self.bootstrap_initial_worker().await?;
        self.start_worker_supervisor();

        let app = Router::new()
            .route("/healthz", get(splitter_healthz))
            .route("/readyz", get(splitter_readyz))
            .route(SPLITTER_STATUS_PATH, get(splitter_status))
            .route(SPLITTER_RELOAD_PATH, post(splitter_reload))
            .route("/v1/realtime", get(proxy_websocket_request))
            .route(
                "/ws/google.ai.generativelanguage.v1alpha.GenerativeService/BidiGenerateContent",
                get(proxy_websocket_request),
            )
            .route(
                "/ws/google.ai.generativelanguage.v1beta.GenerativeService/BidiGenerateContent",
                get(proxy_websocket_request),
            )
            .layer(middleware::from_fn_with_state(
                self.clone(),
                ensure_worker_available,
            ))
            .fallback(any(proxy_request))
            .with_state(self.clone());

        tracing::info!(port = self.inner.config.port, "Gateway splitter listening");

        let shutdown_manager = self.clone();
        let serve_result = axum::serve(listener, app)
            .with_graceful_shutdown(splitter_shutdown_signal(shutdown_manager.clone()))
            .await;

        shutdown_manager
            .inner
            .supervisor_shutdown
            .store(true, Ordering::SeqCst);
        shutdown_manager.shutdown_all_workers().await;
        tracing::info!("Gateway splitter shut down complete");
        serve_result.context("gateway splitter server failed")?;
        Ok(())
    }

    fn start_worker_supervisor(&self) {
        self.inner
            .supervisor_shutdown
            .store(false, Ordering::SeqCst);
        let manager = self.clone();
        tokio::spawn(async move {
            while !manager.inner.supervisor_shutdown.load(Ordering::SeqCst) {
                manager.reconcile_worker_processes().await;
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        });
    }

    async fn reconcile_worker_processes(&self) {
        let workers: Vec<_> = self.inner.workers.read().values().cloned().collect();
        for worker in workers {
            let mut child_guard = worker.child.lock().await;
            let Some(child) = child_guard.as_mut() else {
                continue;
            };
            let exit_status = match child.try_wait() {
                Ok(Some(status)) => Some(status.to_string()),
                Ok(None) => None,
                Err(error) => Some(format!("wait_error: {error}")),
            };
            let Some(exit_status) = exit_status else {
                continue;
            };

            let was_active = worker.status.read().can_receive_traffic();
            let current_status = *worker.status.read();
            if current_status != SplitterWorkerStatus::Exited {
                let _ = worker.transition_to(SplitterWorkerStatus::Exited);
            }
            *worker.exit_status.write() = Some(if was_active {
                format!("crashed: {exit_status}")
            } else {
                exit_status
            });
            *child_guard = None;
            let clears_active =
                self.inner.active_worker_id.read().as_deref() == Some(worker.id.as_str());
            if clears_active {
                *self.inner.active_worker_id.write() = None;
            }
            worker.request_notify.notify_waiters();
        }
    }

    async fn bootstrap_initial_worker(&self) -> anyhow::Result<()> {
        let worker = self
            .spawn_worker(
                self.inner.config.splitter_worker_executable_path.clone(),
                None,
            )
            .await
            .context("failed to spawn initial gateway worker")?;
        if let Err(error) = self
            .wait_for_worker_ready(
                worker.as_ref(),
                self.inner.config.splitter_ready_timeout_secs,
            )
            .await
        {
            if let Err(cleanup_error) = self
                .terminate_unready_worker(Arc::clone(&worker), "initial_readiness_failed")
                .await
            {
                tracing::warn!(worker_id = %worker.id, ?cleanup_error, "Failed to terminate unready initial gateway worker");
            }
            return Err(error).context("initial gateway worker did not become ready");
        }
        self.set_active_worker(worker)?;
        Ok(())
    }

    async fn reload_worker(
        &self,
        request: ReloadSplitterRequest,
    ) -> anyhow::Result<serde_json::Value> {
        let _guard = self.inner.reload_lock.lock().await;

        let worker = self
            .spawn_worker(request.executable_path.clone(), request.worker_port)
            .await
            .context("failed to spawn replacement gateway worker")?;

        if let Err(error) = self
            .wait_for_worker_ready(
                worker.as_ref(),
                request
                    .ready_timeout_secs
                    .unwrap_or(self.inner.config.splitter_ready_timeout_secs),
            )
            .await
        {
            if let Err(cleanup_error) = self
                .terminate_unready_worker(Arc::clone(&worker), "replacement_readiness_failed")
                .await
            {
                tracing::warn!(worker_id = %worker.id, ?cleanup_error, "Failed to terminate unready replacement gateway worker");
            }
            return Err(error).context("replacement gateway worker did not become ready");
        }

        let previous = self.set_active_worker(Arc::clone(&worker))?;
        if let Some(old_worker) = previous {
            let shutdown_timeout_secs = request
                .shutdown_timeout_secs
                .unwrap_or(self.inner.config.splitter_reload_shutdown_timeout_secs);
            self.mark_worker_draining(old_worker.as_ref(), "splitter_reload")?;
            let drain_manager = self.clone();
            tokio::spawn(async move {
                if let Err(error) = drain_manager
                    .drain_and_wait_for_exit(old_worker, shutdown_timeout_secs)
                    .await
                {
                    tracing::warn!(?error, "Failed to drain previous gateway worker");
                }
            });
        }

        Ok(self.status_payload())
    }

    async fn spawn_worker(
        &self,
        executable_path: Option<String>,
        worker_port: Option<u16>,
    ) -> anyhow::Result<Arc<ManagedWorker>> {
        let executable_path = executable_path
            .or_else(|| self.inner.config.splitter_worker_executable_path.clone())
            .unwrap_or_else(|| {
                std::env::current_exe()
                    .ok()
                    .map(|path| path.display().to_string())
                    .unwrap_or_else(|| "gateway".to_string())
            });

        let port = worker_port.unwrap_or_else(|| self.allocate_worker_port());
        let id = format!("worker-{port}-{}", Uuid::new_v4());

        tracing::info!(worker_id = %id, port, executable_path = %executable_path, "Spawning gateway worker");

        let mut command = Command::new(&executable_path);
        command.env("GATEWAY_RUNTIME_ROLE", "worker");
        command.env("GATEWAY_SPLITTER_WORKER_ID", &id);
        command.env("PORT", port.to_string());
        command.stdin(Stdio::null());
        command.stdout(Stdio::inherit());
        command.stderr(Stdio::inherit());
        command.kill_on_drop(true);

        let child = command
            .spawn()
            .with_context(|| format!("failed to spawn worker executable {}", executable_path))?;
        let pid = child.id();

        let worker = Arc::new(ManagedWorker {
            id: id.clone(),
            port,
            base_url: format!("http://127.0.0.1:{port}"),
            executable_path,
            started_at: now_rfc3339(),
            status: RwLock::new(SplitterWorkerStatus::Starting),
            drain_requested_at: RwLock::new(None),
            drain_reason: RwLock::new(None),
            exit_status: RwLock::new(None),
            child: Mutex::new(Some(child)),
            active_requests: AtomicUsize::new(0),
            request_notify: Notify::new(),
            pid,
        });

        self.inner.workers.write().insert(id, Arc::clone(&worker));
        Ok(worker)
    }

    fn allocate_worker_port(&self) -> u16 {
        self.inner.next_worker_port.fetch_add(1, Ordering::SeqCst) as u16
    }

    async fn wait_for_worker_ready(
        &self,
        worker: &ManagedWorker,
        timeout_secs: u64,
    ) -> anyhow::Result<()> {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(timeout_secs.max(1));
        let poll_interval = Duration::from_millis(
            self.inner
                .config
                .splitter_ready_poll_interval_millis
                .max(100),
        );

        loop {
            if let Some(exit_status) = self.worker_exit_status(worker).await? {
                return Err(anyhow!(
                    "worker {} on port {} exited before readiness: {}",
                    worker.id,
                    worker.port,
                    exit_status
                ));
            }

            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                return Err(anyhow!(
                    "worker {} on port {} did not become ready in time",
                    worker.id,
                    worker.port
                ));
            }

            let probe_timeout = remaining.min(poll_interval);
            if matches!(
                tokio::time::timeout(probe_timeout, self.worker_is_ready(worker)).await,
                Ok(true)
            ) {
                self.transition_worker_status(worker, SplitterWorkerStatus::Ready)?;
                return Ok(());
            }

            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                return Err(anyhow!(
                    "worker {} on port {} did not become ready in time",
                    worker.id,
                    worker.port
                ));
            }

            tokio::time::sleep(remaining.min(poll_interval)).await;
        }
    }

    async fn worker_exit_status(&self, worker: &ManagedWorker) -> anyhow::Result<Option<String>> {
        let mut child_guard = worker.child.lock().await;
        let Some(child) = child_guard.as_mut() else {
            return Ok(Some("child_missing".to_string()));
        };
        child
            .try_wait()
            .map(|status| status.map(|status| status.to_string()))
            .with_context(|| format!("failed to inspect worker {} process state", worker.id))
    }

    async fn worker_is_ready(&self, worker: &ManagedWorker) -> bool {
        match self
            .inner
            .ready_client
            .get(format!("{}/readyz", worker.base_url))
            .send()
            .await
        {
            Ok(response) => {
                let response_status = response.status();
                let identity_matches = response
                    .headers()
                    .get("x-gateway-worker-id")
                    .and_then(|value| value.to_str().ok())
                    .map(str::trim)
                    == Some(worker.id.as_str());
                let body_identity_matches = response
                    .json::<serde_json::Value>()
                    .await
                    .ok()
                    .and_then(|payload| {
                        payload
                            .get("worker_id")
                            .and_then(serde_json::Value::as_str)
                            .map(str::trim)
                            .map(str::to_string)
                    })
                    .as_deref()
                    == Some(worker.id.as_str());
                response_status.is_success()
                    && identity_matches
                    && body_identity_matches
                    && matches!(self.worker_exit_status(worker).await, Ok(None))
            }
            Err(_) => false,
        }
    }

    fn set_active_worker(
        &self,
        worker: Arc<ManagedWorker>,
    ) -> anyhow::Result<Option<Arc<ManagedWorker>>> {
        let previous = self.active_worker();
        self.transition_worker_status(worker.as_ref(), SplitterWorkerStatus::Active)?;
        *self.inner.active_worker_id.write() = Some(worker.id.clone());
        Ok(previous.filter(|old| old.id != worker.id && old.status.read().can_receive_traffic()))
    }

    fn active_worker(&self) -> Option<Arc<ManagedWorker>> {
        let active_worker_id = self.inner.active_worker_id.read().clone()?;
        self.inner.workers.read().get(&active_worker_id).cloned()
    }

    fn routing_worker(&self) -> Option<Arc<ManagedWorker>> {
        let worker = self.active_worker()?;
        if !worker.status.read().can_receive_traffic() {
            return None;
        }
        Some(worker)
    }

    fn mark_worker_draining(&self, worker: &ManagedWorker, reason: &str) -> anyhow::Result<()> {
        self.transition_worker_status(worker, SplitterWorkerStatus::Draining)?;
        *worker.drain_requested_at.write() = Some(now_rfc3339());
        *worker.drain_reason.write() = Some(reason.to_string());
        Ok(())
    }

    fn transition_worker_status(
        &self,
        worker: &ManagedWorker,
        next: SplitterWorkerStatus,
    ) -> anyhow::Result<()> {
        worker.transition_to(next)
    }

    async fn terminate_unready_worker(
        &self,
        worker: Arc<ManagedWorker>,
        reason: &str,
    ) -> anyhow::Result<()> {
        let mut child_guard = worker.child.lock().await;
        let exit_status = if let Some(child) = child_guard.as_mut() {
            let _ = child.start_kill();
            match tokio::time::timeout(Duration::from_secs(2), child.wait()).await {
                Ok(Ok(status)) => format!("{reason}: {status}"),
                Ok(Err(error)) => format!("{reason}: wait_error: {error}"),
                Err(_) => {
                    return Err(anyhow!(
                        "{reason}: worker {} did not terminate during cleanup",
                        worker.id
                    ));
                }
            }
        } else {
            format!("{reason}: child_missing")
        };

        if *worker.status.read() != SplitterWorkerStatus::Exited {
            self.transition_worker_status(worker.as_ref(), SplitterWorkerStatus::Exited)?;
        }
        *worker.exit_status.write() = Some(exit_status);
        *child_guard = None;
        Ok(())
    }

    async fn drain_and_wait_for_exit(
        &self,
        worker: Arc<ManagedWorker>,
        shutdown_timeout_secs: u64,
    ) -> anyhow::Result<()> {
        let total_budget = Duration::from_secs(shutdown_timeout_secs.max(1));
        let hard_deadline = tokio::time::Instant::now() + total_budget;
        let kill_grace = (total_budget / 4)
            .max(Duration::from_millis(100))
            .min(Duration::from_secs(2));
        let graceful_deadline = hard_deadline - kill_grace;

        let requests_drained = worker
            .wait_for_no_active_requests_until(graceful_deadline)
            .await;
        if requests_drained {
            let drain_reason = worker
                .drain_reason
                .read()
                .clone()
                .unwrap_or_else(|| "splitter_shutdown".to_string());
            let drain_request = self
                .inner
                .ready_client
                .post(format!(
                    "{}/v1/internal/gateway/runtime/drain",
                    worker.base_url
                ))
                .header(
                    "x-management-token",
                    self.inner
                        .config
                        .gateway_management_token
                        .clone()
                        .unwrap_or_default(),
                )
                .json(&serde_json::json!({ "reason": drain_reason }))
                .send();
            match tokio::time::timeout_at(graceful_deadline, drain_request).await {
                Ok(Ok(response))
                    if response.status().is_success()
                        || response.status() == StatusCode::ACCEPTED =>
                {
                    tracing::info!(worker_id = %worker.id, "Requested graceful drain for gateway worker");
                }
                Ok(Ok(response)) => {
                    tracing::warn!(worker_id = %worker.id, status = %response.status(), "Gateway worker drain request returned non-success status");
                }
                Ok(Err(error)) => {
                    tracing::warn!(worker_id = %worker.id, ?error, "Gateway worker drain request failed");
                }
                Err(_) => {
                    tracing::warn!(worker_id = %worker.id, "Gateway worker drain request exceeded the shutdown deadline");
                }
            }
        } else {
            tracing::warn!(
                worker_id = %worker.id,
                active_requests = worker.active_requests(),
                "Gateway worker still has in-flight splitter requests at the graceful shutdown deadline"
            );
        }

        let Some(mut child) = worker.child.lock().await.take() else {
            if *worker.status.read() != SplitterWorkerStatus::Exited {
                self.transition_worker_status(worker.as_ref(), SplitterWorkerStatus::Exited)?;
            }
            return Ok(());
        };

        if requests_drained {
            match tokio::time::timeout_at(graceful_deadline, child.wait()).await {
                Ok(Ok(status)) => {
                    self.transition_worker_status(worker.as_ref(), SplitterWorkerStatus::Exited)?;
                    *worker.exit_status.write() = Some(status.to_string());
                    return Ok(());
                }
                Ok(Err(error)) => {
                    self.transition_worker_status(worker.as_ref(), SplitterWorkerStatus::Exited)?;
                    *worker.exit_status.write() = Some(format!("wait_error: {error}"));
                    return Ok(());
                }
                Err(_) => {}
            }
        }

        tracing::warn!(worker_id = %worker.id, "Gateway worker exceeded graceful shutdown; forcing kill");
        let _ = child.start_kill();
        match tokio::time::timeout_at(hard_deadline, child.wait()).await {
            Ok(Ok(status)) => {
                self.transition_worker_status(worker.as_ref(), SplitterWorkerStatus::Exited)?;
                *worker.exit_status.write() = Some(format!("forced_kill: {status}"));
                Ok(())
            }
            Ok(Err(error)) => {
                self.transition_worker_status(worker.as_ref(), SplitterWorkerStatus::Exited)?;
                *worker.exit_status.write() = Some(format!("forced_kill_wait_error: {error}"));
                Ok(())
            }
            Err(_) => {
                *worker.exit_status.write() = Some("forced_kill_pending".to_string());
                *worker.child.lock().await = Some(child);
                Err(anyhow!(
                    "worker {} did not terminate within {} seconds",
                    worker.id,
                    shutdown_timeout_secs.max(1)
                ))
            }
        }
    }

    async fn shutdown_all_workers(&self) {
        let workers: Vec<_> = self.inner.workers.read().values().cloned().collect();
        for worker in workers {
            if *worker.status.read() != SplitterWorkerStatus::Exited {
                if let Err(error) = self.mark_worker_draining(worker.as_ref(), "splitter_shutdown")
                {
                    tracing::warn!(worker_id = %worker.id, ?error, "Failed to mark worker draining during splitter exit");
                }
            }
            if let Err(error) = self
                .drain_and_wait_for_exit(
                    worker,
                    self.inner.config.splitter_reload_shutdown_timeout_secs,
                )
                .await
            {
                tracing::warn!(?error, "Failed to shut down worker during splitter exit");
            }
        }
    }

    fn status_payload(&self) -> serde_json::Value {
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

    fn management_token(&self) -> Option<&str> {
        self.inner.config.gateway_management_token.as_deref()
    }

    fn max_request_body_bytes(&self) -> usize {
        self.inner.config.max_request_body_bytes
    }
}

async fn splitter_shutdown_signal(manager: SplitterManager) {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};

        let ctrl_c = tokio::signal::ctrl_c();
        let mut sigterm =
            signal(SignalKind::terminate()).expect("failed to install SIGTERM handler");

        tokio::select! {
            _ = ctrl_c => {
                tracing::info!("Received SIGINT, shutting down gateway splitter...");
            }
            _ = sigterm.recv() => {
                tracing::info!("Received SIGTERM, shutting down gateway splitter...");
            }
        }
    }

    #[cfg(not(unix))]
    {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to listen for ctrl_c");
        tracing::info!("Received shutdown signal, shutting down gateway splitter...");
    }
    let _ = manager;
}

async fn ensure_worker_available(
    State(manager): State<SplitterManager>,
    request: Request,
    next: Next,
) -> Response {
    let path = request.uri().path();
    if path == "/healthz"
        || path == "/readyz"
        || path == SPLITTER_STATUS_PATH
        || path == SPLITTER_RELOAD_PATH
    {
        return next.run(request).await;
    }

    if manager.routing_worker().is_none() {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({
                "error": {
                    "message": "Gateway splitter has no available worker",
                    "code": "gateway_splitter_unavailable",
                }
            })),
        )
            .into_response();
    }

    next.run(request).await
}

async fn splitter_healthz() -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "status": "ok",
            "role": "splitter",
        })),
    )
}

async fn splitter_readyz(State(manager): State<SplitterManager>) -> impl IntoResponse {
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

fn splitter_readiness_endpoint_timeout() -> Duration {
    std::env::var("GATEWAY_SPLITTER_READINESS_ENDPOINT_TIMEOUT_MS")
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .map(|value| value.clamp(50, 5_000))
        .map(Duration::from_millis)
        .unwrap_or_else(|| Duration::from_millis(1_500))
}

async fn splitter_status(
    State(manager): State<SplitterManager>,
    headers: HeaderMap,
) -> impl IntoResponse {
    if let Err(response) = assert_splitter_management_access(&manager, &headers) {
        return response;
    }
    manager.reconcile_worker_processes().await;

    (StatusCode::OK, Json(manager.status_payload())).into_response()
}

async fn splitter_reload(
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

async fn proxy_request(
    State(manager): State<SplitterManager>,
    request: Request<Body>,
) -> impl IntoResponse {
    manager.reconcile_worker_processes().await;
    let Some(worker) = manager.routing_worker() else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({
                "error": {
                    "message": "Gateway splitter has no available worker",
                    "code": "gateway_splitter_unavailable",
                }
            })),
        )
            .into_response();
    };
    let Some(lease) = worker.try_acquire_lease() else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({
                "error": {
                    "message": "Gateway splitter worker entered drain before request admission",
                    "code": "gateway_splitter_unavailable",
                }
            })),
        )
            .into_response();
    };

    let (parts, body) = request.into_parts();
    let body_bytes = match to_bytes(body, manager.max_request_body_bytes()).await {
        Ok(bytes) => bytes,
        Err(error) => {
            return (
                StatusCode::PAYLOAD_TOO_LARGE,
                Json(serde_json::json!({
                    "error": {
                        "message": format!("Request body too large: {}", error),
                        "code": "request_too_large",
                    }
                })),
            )
                .into_response();
        }
    };

    match send_request_to_worker(&manager, worker.as_ref(), &parts, body_bytes, lease).await {
        Ok(response) => response,
        Err(error) => {
            tracing::warn!(
                worker_id = %worker.id,
                ?error,
                "Failed to proxy request to active worker"
            );
            (
                StatusCode::BAD_GATEWAY,
                Json(serde_json::json!({
                    "error": {
                        "message": error.to_string(),
                        "code": "gateway_splitter_proxy_failed",
                    }
                })),
            )
                .into_response()
        }
    }
}

async fn proxy_websocket_request(
    ws: WebSocketUpgrade,
    OriginalUri(uri): OriginalUri,
    State(manager): State<SplitterManager>,
    headers: HeaderMap,
) -> impl IntoResponse {
    manager.reconcile_worker_processes().await;
    let Some(worker) = manager.routing_worker() else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({
                "error": {
                    "message": "Gateway splitter has no available worker",
                    "code": "gateway_splitter_unavailable",
                }
            })),
        )
            .into_response();
    };
    let Some(lease) = worker.try_acquire_lease() else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({
                "error": {
                    "message": "Gateway splitter worker entered drain before websocket admission",
                    "code": "gateway_splitter_unavailable",
                }
            })),
        )
            .into_response();
    };

    let worker_id = worker.id.clone();
    ws.on_upgrade(move |socket| async move {
        if let Err(error) = bridge_websocket_to_worker(worker, socket, uri, headers, lease).await {
            tracing::warn!(
                worker_id = %worker_id,
                ?error,
                "Failed to proxy websocket request to active worker"
            );
        }
    })
    .into_response()
}

async fn bridge_websocket_to_worker(
    worker: Arc<ManagedWorker>,
    socket: WebSocket,
    uri: Uri,
    headers: HeaderMap,
    _lease: WorkerRequestLease,
) -> anyhow::Result<()> {
    let target = format!(
        "{}{}",
        worker_ws_base_url(&worker.base_url),
        uri.path_and_query()
            .map(|value| value.as_str())
            .unwrap_or("/")
    );

    let mut upstream_request = target
        .into_client_request()
        .context("failed to build websocket worker request")?;
    for (name, value) in &headers {
        if !is_hop_by_hop_header(name) && !is_websocket_handshake_header(name) {
            upstream_request
                .headers_mut()
                .insert(name.clone(), value.clone());
        }
    }
    upstream_request.headers_mut().insert(
        HeaderName::from_static("x-gateway-splitter-worker-id"),
        HeaderValue::from_str(worker.id.as_str())
            .unwrap_or_else(|_| HeaderValue::from_static("unknown")),
    );

    let (upstream_socket, _) = connect_async(upstream_request)
        .await
        .context("failed to open websocket connection to worker")?;
    relay_websocket(socket, upstream_socket).await
}

async fn relay_websocket(
    downstream: WebSocket,
    upstream: tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
) -> anyhow::Result<()> {
    let (mut downstream_tx, mut downstream_rx) = downstream.split();
    let (mut upstream_tx, mut upstream_rx) = upstream.split();

    let downstream_to_upstream = async {
        while let Some(message) = downstream_rx.next().await {
            let message = message.context("failed to receive downstream websocket frame")?;
            upstream_tx
                .send(axum_message_to_tungstenite(message))
                .await
                .context("failed to forward downstream websocket frame")?;
        }
        upstream_tx
            .close()
            .await
            .context("failed to close upstream websocket writer")
    };

    let upstream_to_downstream = async {
        while let Some(message) = upstream_rx.next().await {
            let message = message.context("failed to receive upstream websocket frame")?;
            if let Some(message) = tungstenite_message_to_axum(message) {
                downstream_tx
                    .send(message)
                    .await
                    .context("failed to forward upstream websocket frame")?;
            }
        }
        downstream_tx
            .close()
            .await
            .context("failed to close downstream websocket writer")
    };

    let _ = tokio::try_join!(downstream_to_upstream, upstream_to_downstream)?;
    Ok(())
}

fn axum_message_to_tungstenite(message: AxumWsMessage) -> TungsteniteMessage {
    match message {
        AxumWsMessage::Text(text) => TungsteniteMessage::Text(text.to_string().into()),
        AxumWsMessage::Binary(bytes) => TungsteniteMessage::Binary(bytes),
        AxumWsMessage::Ping(bytes) => TungsteniteMessage::Ping(bytes),
        AxumWsMessage::Pong(bytes) => TungsteniteMessage::Pong(bytes),
        AxumWsMessage::Close(frame) => TungsteniteMessage::Close(frame.map(|frame| {
            tokio_tungstenite::tungstenite::protocol::CloseFrame {
                code: frame.code.into(),
                reason: frame.reason.to_string().into(),
            }
        })),
    }
}

fn tungstenite_message_to_axum(message: TungsteniteMessage) -> Option<AxumWsMessage> {
    match message {
        TungsteniteMessage::Text(text) => Some(AxumWsMessage::Text(text)),
        TungsteniteMessage::Binary(bytes) => Some(AxumWsMessage::Binary(bytes)),
        TungsteniteMessage::Ping(bytes) => Some(AxumWsMessage::Ping(bytes)),
        TungsteniteMessage::Pong(bytes) => Some(AxumWsMessage::Pong(bytes)),
        TungsteniteMessage::Close(frame) => Some(AxumWsMessage::Close(frame.map(|frame| {
            axum::extract::ws::CloseFrame {
                code: frame.code.into(),
                reason: frame.reason.to_string().into(),
            }
        }))),
        TungsteniteMessage::Frame(_) => None,
    }
}

fn worker_ws_base_url(base_url: &str) -> String {
    if let Some(rest) = base_url.strip_prefix("https://") {
        return format!("wss://{rest}");
    }
    if let Some(rest) = base_url.strip_prefix("http://") {
        return format!("ws://{rest}");
    }
    base_url.to_string()
}

async fn send_request_to_worker(
    manager: &SplitterManager,
    worker: &ManagedWorker,
    parts: &axum::http::request::Parts,
    body_bytes: bytes::Bytes,
    lease: WorkerRequestLease,
) -> anyhow::Result<Response<Body>> {
    let target = format!(
        "{}{}",
        worker.base_url,
        parts
            .uri
            .path_and_query()
            .map(|value| value.as_str())
            .unwrap_or("/")
    );

    let mut builder = manager
        .inner
        .proxy_client
        .request(parts.method.clone(), target);
    for (name, value) in &parts.headers {
        if !is_hop_by_hop_header(name) {
            builder = builder.header(name, value.clone());
        }
    }
    builder = builder.header("x-gateway-splitter-worker-id", worker.id.as_str());
    builder = builder.body(body_bytes);

    let upstream = builder.send().await?;
    let status = upstream.status();
    let upstream_headers = upstream.headers().clone();
    let stream = Box::pin(upstream.bytes_stream());
    let stream = futures::stream::unfold((stream, lease), |(mut stream, lease)| async move {
        stream.next().await.map(|item| {
            (
                item.map_err(|error| std::io::Error::other(error.to_string())),
                (stream, lease),
            )
        })
    });

    let mut response = Response::builder()
        .status(status)
        .body(Body::from_stream(stream))
        .map_err(|error| anyhow!("failed to build worker response: {}", error))?;

    for (name, value) in &upstream_headers {
        if !is_hop_by_hop_header(name) {
            response.headers_mut().append(name, value.clone());
        }
    }
    response.headers_mut().insert(
        HeaderName::from_static("x-gateway-splitter-worker-id"),
        HeaderValue::from_str(worker.id.as_str())
            .unwrap_or_else(|_| HeaderValue::from_static("unknown")),
    );

    Ok(response)
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

fn is_hop_by_hop_header(name: &HeaderName) -> bool {
    matches!(
        *name,
        CONNECTION
            | HOST
            | PROXY_AUTHENTICATE
            | PROXY_AUTHORIZATION
            | TE
            | TRAILER
            | TRANSFER_ENCODING
            | UPGRADE
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_worker() -> Arc<ManagedWorker> {
        Arc::new(ManagedWorker {
            id: "worker-test".to_string(),
            port: 4201,
            base_url: "http://127.0.0.1:4201".to_string(),
            executable_path: "gateway".to_string(),
            started_at: "test".to_string(),
            status: RwLock::new(SplitterWorkerStatus::Active),
            drain_requested_at: RwLock::new(None),
            drain_reason: RwLock::new(None),
            exit_status: RwLock::new(None),
            child: Mutex::new(None),
            active_requests: AtomicUsize::new(0),
            request_notify: tokio::sync::Notify::new(),
            pid: None,
        })
    }

    #[tokio::test]
    async fn worker_lease_admission_closes_on_drain_and_waits_for_release() {
        let worker = test_worker();
        let lease = worker
            .try_acquire_lease()
            .expect("active worker must admit a request");
        assert_eq!(worker.active_requests(), 1);

        assert!(worker.transition_to(SplitterWorkerStatus::Draining).is_ok());
        assert!(worker.try_acquire_lease().is_none());

        let wait = worker.wait_for_no_active_requests(Duration::from_secs(1));
        tokio::pin!(wait);
        assert!(tokio::time::timeout(Duration::from_millis(20), &mut wait)
            .await
            .is_err());

        drop(lease);
        assert!(wait.await);
        assert_eq!(worker.active_requests(), 0);
    }

    #[tokio::test]
    async fn splitter_readiness_probe_timeout_is_bounded() {
        let started = tokio::time::Instant::now();
        let result = tokio::time::timeout(Duration::from_millis(20), async {
            std::future::pending::<bool>().await
        })
        .await;

        assert!(result.is_err());
        assert!(started.elapsed() < Duration::from_secs(1));
        assert!(splitter_readiness_endpoint_timeout() <= Duration::from_secs(5));
    }
}

fn is_websocket_handshake_header(name: &HeaderName) -> bool {
    let lower = name.as_str().to_ascii_lowercase();
    lower == "sec-websocket-key"
        || lower == "sec-websocket-version"
        || lower == "sec-websocket-extensions"
        || lower == "sec-websocket-protocol"
        || lower == "sec-websocket-accept"
}

fn now_rfc3339() -> String {
    OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "unknown".to_string())
}
