// ---------------------------------------------------------------------------
// AppState — shared gateway application state
// ---------------------------------------------------------------------------

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

use parking_lot::RwLock;
use time::OffsetDateTime;
use tokio::sync::{watch, Mutex, Notify, OwnedMutexGuard};

use crate::auth::adapter::AuthAdapter;
use crate::concurrency::registry::ConcurrencyRegistry;
use crate::config::Config;
use crate::console::{ConsoleAuthRuntime, RouteConfigRuntime};
use crate::credential_store::CredentialMemoryCache;
use crate::filter::chain::FilterChainConfig;
use crate::redis::pool::RedisPool;
use crate::routing::config::RouteConfigStore;
use crate::upstream::client::UpstreamClient;
use sqlx::PgPool;

#[derive(Debug, Default)]
pub struct GatewayLifecycleState {
    draining: AtomicBool,
    active_requests: AtomicUsize,
    drain_started_at: RwLock<Option<String>>,
    drain_reason: RwLock<Option<String>>,
    provider_deletion: Arc<Mutex<()>>,
}

impl GatewayLifecycleState {
    pub fn begin_drain(&self, reason: &str) -> bool {
        let started = !self.draining.swap(true, Ordering::SeqCst);
        if started {
            *self.drain_started_at.write() = Some(
                OffsetDateTime::now_utc()
                    .format(&time::format_description::well_known::Rfc3339)
                    .unwrap_or_else(|_| "unknown".to_string()),
            );
            *self.drain_reason.write() = Some(reason.trim().to_string());
        }
        started
    }

    pub fn is_draining(&self) -> bool {
        self.draining.load(Ordering::SeqCst)
    }

    pub fn active_requests(&self) -> usize {
        self.active_requests.load(Ordering::SeqCst)
    }

    pub fn begin_request(&self) {
        self.active_requests.fetch_add(1, Ordering::SeqCst);
    }

    pub fn end_request(&self) {
        self.active_requests.fetch_sub(1, Ordering::SeqCst);
    }

    pub(crate) fn try_begin_provider_deletion(&self) -> Option<OwnedMutexGuard<()>> {
        self.provider_deletion.clone().try_lock_owned().ok()
    }

    pub fn drain_started_at(&self) -> Option<String> {
        self.drain_started_at.read().clone()
    }

    pub fn drain_reason(&self) -> Option<String> {
        self.drain_reason.read().clone()
    }
}

#[derive(Clone, Debug, Default)]
pub struct GatewayShutdownHandle {
    requested: Arc<AtomicBool>,
    requested_at: Arc<RwLock<Option<String>>>,
    reason: Arc<RwLock<Option<String>>>,
    notify: Arc<Notify>,
}

impl GatewayShutdownHandle {
    pub fn request(&self, reason: &str) -> bool {
        let started = !self.requested.swap(true, Ordering::SeqCst);
        if started {
            *self.requested_at.write() = Some(
                OffsetDateTime::now_utc()
                    .format(&time::format_description::well_known::Rfc3339)
                    .unwrap_or_else(|_| "unknown".to_string()),
            );
            *self.reason.write() = Some(reason.trim().to_string());
        }
        self.notify.notify_waiters();
        started
    }

    pub fn is_requested(&self) -> bool {
        self.requested.load(Ordering::SeqCst)
    }

    pub async fn wait(&self) {
        // notify_waiters remembers futures created before the request, even unpolled.
        let notified = self.notify.notified();
        if self.is_requested() {
            return;
        }
        notified.await;
    }

    pub fn requested_at(&self) -> Option<String> {
        self.requested_at.read().clone()
    }

    pub fn reason(&self) -> Option<String> {
        self.reason.read().clone()
    }
}

#[derive(Clone, Debug)]
pub struct ProviderCredentialFolderSyncSnapshot {
    enabled: bool,
    epoch: Arc<()>,
}

impl ProviderCredentialFolderSyncSnapshot {
    pub fn enabled(&self) -> bool {
        self.enabled
    }

    pub(crate) fn same_epoch(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.epoch, &other.epoch)
    }
}

#[derive(Clone, Debug)]
pub struct ProviderCredentialFolderSyncRuntime {
    tx: watch::Sender<ProviderCredentialFolderSyncSnapshot>,
    enable_update: Arc<Mutex<()>>,
    sync_run: Arc<Mutex<()>>,
}

impl ProviderCredentialFolderSyncRuntime {
    pub fn new(enabled: bool) -> Self {
        let (tx, _rx) = watch::channel(ProviderCredentialFolderSyncSnapshot {
            enabled,
            epoch: Arc::new(()),
        });
        Self {
            tx,
            enable_update: Arc::new(Mutex::new(())),
            sync_run: Arc::new(Mutex::new(())),
        }
    }

    pub(crate) async fn begin_enable_update(&self) -> OwnedMutexGuard<()> {
        self.enable_update.clone().lock_owned().await
    }

    pub(crate) fn try_begin_sync_run(&self) -> Option<OwnedMutexGuard<()>> {
        self.sync_run.clone().try_lock_owned().ok()
    }

    pub fn enabled(&self) -> bool {
        self.tx.borrow().enabled
    }

    pub(crate) fn snapshot(&self) -> ProviderCredentialFolderSyncSnapshot {
        self.tx.borrow().clone()
    }

    pub fn set_enabled(&self, enabled: bool) -> bool {
        self.tx.send_if_modified(|state| {
            if state.enabled == enabled {
                return false;
            }
            // Retained old snapshots prevent identity reuse across a coalesced disable.
            if !enabled {
                state.epoch = Arc::new(());
            }
            state.enabled = enabled;
            true
        })
    }

    pub fn subscribe(&self) -> watch::Receiver<ProviderCredentialFolderSyncSnapshot> {
        self.tx.subscribe()
    }
}

/// Application state shared across all request handlers via [`Arc`].
///
/// Constructed once at startup and injected into the axum router via
/// [`axum::extract::State`].
pub struct AppState {
    pub config: Config,
    pub redis_pool: RedisPool,
    pub pg_pool: Option<PgPool>,
    pub upstream_client: UpstreamClient,
    pub concurrency_registry: ConcurrencyRegistry,
    /// Ordered chain of auth adapters tried in sequence for each request.
    pub auth_adapters: Vec<Box<dyn AuthAdapter>>,
    /// Optional content filter configuration.  `None` means filtering is
    /// disabled.
    pub filter_config: Option<FilterChainConfig>,
    /// Route configuration: providers, model routes, and aliases.
    pub route_config: Arc<RouteConfigStore>,
    /// Optional transactional route configuration runtime used by the web console.
    pub route_config_runtime: Option<Arc<RouteConfigRuntime>>,
    /// Browser console bootstrap/session/auth runtime.
    pub console_auth: Arc<ConsoleAuthRuntime>,
    /// In-memory credential cache (Tier 1). TTL-based, per-process.
    pub credential_cache: CredentialMemoryCache,
    /// Process lifecycle and in-flight request tracking used for graceful
    /// draining during rolling deployments.
    pub lifecycle: GatewayLifecycleState,
    /// Programmatic shutdown hook used by internal runtime drain requests and
    /// the splitter supervisor.
    pub shutdown: GatewayShutdownHandle,
    /// Runtime toggle for provider credential folder sync auto mode.
    pub provider_credential_folder_sync: ProviderCredentialFolderSyncRuntime,
    /// Trusted script/HTTP automation registry and per-provider run status.
    pub credential_pool_automation:
        Arc<crate::credential_pool_automation::CredentialPoolAutomationRuntime>,
}

// SAFETY: AppState only contains Send+Sync-safe types.
// FilterChainConfig holds `Box<dyn ContentFilter>` — the trait requires Send+Sync
// (the concrete implementations are thread-safe).
unsafe impl Send for AppState {}
unsafe impl Sync for AppState {}
