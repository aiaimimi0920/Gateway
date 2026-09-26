use crate::config::Config;
use anyhow::anyhow;
use parking_lot::RwLock;
use rquest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
#[cfg(test)]
use std::time::Duration;
use time::OffsetDateTime;
use tokio::process::Child;
use tokio::sync::{Mutex, Notify};
mod draining;
mod http_proxy;
mod lifecycle;
mod management;
mod service;
mod supervisor;
mod websocket_proxy;
#[cfg(test)]
use management::splitter_readiness_endpoint_timeout;
#[cfg(test)]
mod tests;

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

    fn management_token(&self) -> Option<&str> {
        self.inner.config.gateway_management_token.as_deref()
    }

    fn max_request_body_bytes(&self) -> usize {
        self.inner.config.max_request_body_bytes
    }
}

fn now_rfc3339() -> String {
    OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "unknown".to_string())
}
