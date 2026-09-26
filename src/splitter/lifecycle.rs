use super::{
    now_rfc3339, ManagedWorker, ReloadSplitterRequest, SplitterManager, SplitterWorkerStatus,
};
use anyhow::{anyhow, Context};
use parking_lot::RwLock;
use std::process::Stdio;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::process::Command;
use tokio::sync::{Mutex, Notify};
use uuid::Uuid;
impl SplitterManager {
    pub(super) async fn bootstrap_initial_worker(&self) -> anyhow::Result<()> {
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

    pub(super) async fn reload_worker(
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

    pub(super) async fn worker_is_ready(&self, worker: &ManagedWorker) -> bool {
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
}
