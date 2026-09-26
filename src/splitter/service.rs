use super::http_proxy::{ensure_worker_available, proxy_request};
use super::management::{splitter_healthz, splitter_readyz, splitter_reload, splitter_status};
use super::websocket_proxy::proxy_websocket_request;
use super::{SplitterInner, SplitterManager, SPLITTER_RELOAD_PATH, SPLITTER_STATUS_PATH};
use crate::config::Config;
use anyhow::Context;
use axum::middleware;
use axum::routing::{any, get, post};
use axum::Router;
use parking_lot::RwLock;
use rquest::Client;
use rquest_util::Emulation;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
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
