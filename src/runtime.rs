use std::sync::Arc;

use crate::auth::adapter::AuthAdapter;
use crate::auth::project_api_key::ProjectApiKeyAdapter;
use crate::auth::user_credential::UserCredentialAdapter;
use crate::concurrency::aimd::AimdConfig;
use crate::concurrency::registry::ConcurrencyRegistry;
use crate::config::{Config, GatewayStorageMode};
use crate::console::{ConsoleAuthRuntime, RouteConfigRedisStore, RouteConfigRuntime};
use crate::credential_store::CredentialMemoryCache;
use crate::db::create_pg_pool;
use crate::redis::pool::create_pool;
use crate::routing::config::RouteConfigStore;
use crate::state::{
    AppState, GatewayLifecycleState, GatewayShutdownHandle, ProviderCredentialFolderSyncRuntime,
};
use crate::upstream::client::UpstreamClient;

#[path = "runtime_background_tasks.rs"]
pub(crate) mod background_tasks;
use background_tasks::Worker;

#[path = "runtime_shutdown.rs"]
mod shutdown;

#[path = "runtime_completion.rs"]
mod completion;

#[cfg(test)]
#[path = "runtime_lifecycle_tests.rs"]
mod lifecycle_tests;

pub async fn run_gateway_runtime(config: Config) -> anyhow::Result<()> {
    let port = config.port;
    tracing::info!(port, role = ?config.runtime_role, "Starting gateway worker runtime");

    if config.gateway_api_key.is_some() {
        tracing::info!("Gateway API key fallback authentication enabled");
    } else {
        tracing::info!("No GATEWAY_API_KEY set — relying on configured auth adapters");
    }

    let app_state = build_app_state(config).await?;
    spawn_background_tasks(&app_state);
    let freebuff = app_state.upstream_client.freebuff.clone();
    let server_result = run_gateway_server(app_state, port).await;
    completion::complete_after_freebuff_cleanup(
        server_result,
        freebuff.shutdown(std::time::Duration::from_secs(65)),
    )
    .await?;
    tracing::info!(port, "Gateway worker shut down complete");
    Ok(())
}

pub async fn build_app_state(config: Config) -> anyhow::Result<Arc<AppState>> {
    let local_console = config.storage_mode == GatewayStorageMode::Local;
    if local_console && config.runtime_role != crate::config::GatewayRuntimeRole::Standalone {
        anyhow::bail!("Local storage requires standalone runtime role");
    }
    let redis_pool = if local_console {
        crate::redis::pool::disabled_pool()?
    } else {
        create_pool(&config.redis_url)?
    };
    let provider_credential_folder_sync_enabled = if local_console {
        false
    } else {
        crate::provider_credential_folder_sync::load_runtime_enabled(&redis_pool, &config)
            .await
            .unwrap_or_else(|error| {
                tracing::warn!(
                    error = %error.message,
                    "failed to load provider credential folder sync runtime setting; falling back to config"
                );
                crate::provider_credential_folder_sync::default_runtime_enabled(&config)
            })
    };
    let pg_pool = match config.database_url.as_deref().filter(|_| !local_console) {
        Some(database_url) => {
            let pool = create_pg_pool(database_url).await?;
            tracing::info!("PostgreSQL connectivity enabled for Rust-owned gateway state");
            Some(pool)
        }
        None => {
            tracing::debug!(
                "No GATEWAY_DATABASE_URL or DATABASE_URL configured; DB-owned gateway state disabled"
            );
            None
        }
    };

    let upstream_client = UpstreamClient::new_with_runtime(
        config.upstream_timeout_secs,
        (!local_console).then(|| redis_pool.clone()),
        pg_pool.clone(),
    );
    let concurrency_registry = ConcurrencyRegistry::new(AimdConfig::default());
    let mut auth_adapters: Vec<Box<dyn AuthAdapter>> = Vec::new();
    if let (Some(pool), Some(secret)) = (pg_pool.clone(), config.gateway_api_key_secret.clone()) {
        auth_adapters.push(Box::new(ProjectApiKeyAdapter::new(pool, secret)));
    }
    if !local_console {
        auth_adapters.push(Box::new(UserCredentialAdapter::new(
            redis_pool.clone(),
            pg_pool.clone(),
        )));
    }
    let route_config_runtime = Arc::new(if local_console {
        RouteConfigRuntime::new_local(&config.console)?
    } else {
        let route_config = Arc::new(
            load_route_config_from_path(
                &redis_pool,
                &config.console.routes_file,
                &config.console.redis_namespace,
            )
            .await,
        );
        RouteConfigRuntime::new(
            Arc::clone(&route_config),
            &config.console,
            redis_pool.clone(),
            !matches!(
                config.runtime_role,
                crate::config::GatewayRuntimeRole::Worker
            ),
        )?
    });
    let route_config = Arc::clone(route_config_runtime.route_config());
    let local_runtime = if local_console {
        Some(
            crate::local_runtime::LocalRuntime::open(std::path::Path::new(
                &config.console.state_dir,
            ))
            .await?,
        )
    } else {
        None
    };
    let console_auth = Arc::new(ConsoleAuthRuntime::new(
        &config.console,
        config.gateway_management_token.clone(),
    )?);
    if let Err(error) = route_config_runtime.recover_startup().await {
        // A local authority must recover before accepting requests or further writes.
        if local_console {
            return Err(error.into());
        }
        tracing::warn!(
            code = error.code(),
            "Gateway console startup recovery did not fully converge: {}",
            error
        );
    }
    let credential_cache = CredentialMemoryCache::new(30);
    let credential_pool_automation = Arc::new(
        crate::credential_pool_automation::CredentialPoolAutomationRuntime::load(
            &config.credential_pool_automation,
        )?,
    );

    Ok(Arc::new(AppState {
        config,
        redis_pool,
        pg_pool,
        local_runtime,
        upstream_client,
        concurrency_registry,
        routing_latency: Default::default(),
        auth_adapters,
        filter_config: None,
        route_config,
        route_config_runtime: Some(route_config_runtime),
        console_auth,
        credential_cache,
        lifecycle: GatewayLifecycleState::default(),
        shutdown: GatewayShutdownHandle::default(),
        provider_credential_folder_sync: ProviderCredentialFolderSyncRuntime::new(
            provider_credential_folder_sync_enabled,
        ),
        credential_pool_automation,
    }))
}

pub fn spawn_background_tasks(app_state: &Arc<AppState>) {
    app_state
        .lifecycle
        .luma_oauth
        .spawn(app_state.shutdown.clone());
    app_state
        .lifecycle
        .lobsterai_oauth
        .spawn(app_state.shutdown.clone());
    if app_state.shutdown.is_requested() {
        return;
    }
    app_state
        .lifecycle
        .nous_oauth
        .spawn(app_state.shutdown.clone());
    app_state
        .lifecycle
        .dim_oauth
        .spawn(app_state.shutdown.clone());
    app_state
        .lifecycle
        .freebuff_login
        .spawn(app_state.shutdown.clone());
    app_state
        .lifecycle
        .vertex_oauth
        .spawn(app_state.shutdown.clone());
    app_state
        .lifecycle
        .accio_oauth
        .spawn(app_state.shutdown.clone());
    app_state
        .lifecycle
        .kiro_oauth
        .spawn(app_state.shutdown.clone());
    app_state
        .lifecycle
        .grok_oauth
        .spawn(app_state.shutdown.clone());
    crate::token_refresh::spawn(app_state);
    crate::routing::model_circuit::network_probe::schedule(app_state);
    crate::local_runtime::spawn_maintenance(app_state);
    let tasks = &app_state.lifecycle.background_tasks;
    tasks.spawn(
        Worker::CredentialRefill,
        &app_state.shutdown,
        crate::credential_refill::start_credential_refill_notification_task(Arc::clone(app_state)),
    );
    tasks.spawn(
        Worker::CredentialProbe,
        &app_state.shutdown,
        crate::provider_credential_probe_scheduler::start_provider_credential_probe_scheduler(
            Arc::clone(app_state),
        ),
    );
    if app_state.local_runtime.is_some() {
        return;
    }
    crate::provider_credential_refresh::accio::spawn(app_state);
    tasks.spawn(
        Worker::CredentialCache,
        &app_state.shutdown,
        crate::credential_store::start_credential_refresh_task(
            app_state.credential_cache.clone(),
            app_state.redis_pool.clone(),
            10,
            app_state.shutdown.clone(),
        ),
    );
    tasks.spawn(
        Worker::FolderSync,
        &app_state.shutdown,
        crate::provider_credential_folder_sync::start_folder_sync_task(Arc::clone(app_state)),
    );
    tasks.spawn(
        Worker::CredentialRefresh,
        &app_state.shutdown,
        crate::provider_credential_refresh::start_provider_credential_refresh_task(Arc::clone(
            app_state,
        )),
    );
    tasks.spawn(
        Worker::CredentialStock,
        &app_state.shutdown,
        crate::credential_stock::start_credential_stock_monitor_task(Arc::clone(app_state)),
    );
    tasks.spawn(
        Worker::PoolAutomation,
        &app_state.shutdown,
        crate::credential_pool_automation::start_credential_pool_automation_task(Arc::clone(
            app_state,
        )),
    );
}

pub async fn run_gateway_server(app_state: Arc<AppState>, port: u16) -> anyhow::Result<()> {
    let result = serve_gateway_http(&app_state, port).await;
    completion::complete_after_runtime_cleanup(result, shutdown::finish(&app_state)).await
}

async fn serve_gateway_http(app_state: &Arc<AppState>, port: u16) -> anyhow::Result<()> {
    let app = crate::http::router::build_router(Arc::clone(app_state));
    let bind_host: std::net::IpAddr = std::env::var("GATEWAY_BIND_HOST")
        .unwrap_or_else(|_| "0.0.0.0".to_string())
        .parse()?;
    let listener = tokio::net::TcpListener::bind((bind_host, port)).await?;
    tracing::info!(port, "Gateway worker listening");

    let shutdown_state = Arc::clone(app_state);
    // Preserve SocketAddr extraction while giving native upstream pools the
    // same lifetime as the accepted connection, rather than global state.
    let make_service = tower::service_fn(move |stream: axum::serve::IncomingStream<'_>| {
        let app = app
            .clone()
            .layer(axum::Extension(axum::extract::ConnectInfo(
                stream.remote_addr(),
            )))
            .layer(axum::Extension(
                crate::http_client::NativeCodexConnection::default(),
            ));
        async move { Ok::<_, std::convert::Infallible>(app) }
    });
    axum::serve(listener, make_service)
        .with_graceful_shutdown(shutdown_signal(shutdown_state))
        .await?;

    tracing::info!(port, "Gateway HTTP server stopped");
    Ok(())
}

pub async fn shutdown_signal(state: Arc<AppState>) {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};

        let ctrl_c = tokio::signal::ctrl_c();
        let mut sigterm =
            signal(SignalKind::terminate()).expect("failed to install SIGTERM handler");
        let internal_shutdown = state.shutdown.wait();

        tokio::select! {
            _ = ctrl_c => {
                mark_runtime_draining(&state, "sigint");
                tracing::info!("Received SIGINT, shutting down gateway worker gracefully...");
            }
            _ = sigterm.recv() => {
                mark_runtime_draining(&state, "sigterm");
                tracing::info!("Received SIGTERM, shutting down gateway worker gracefully...");
            }
            _ = internal_shutdown => {
                let reason = state.shutdown.reason().unwrap_or_else(|| "internal_shutdown".to_string());
                mark_runtime_draining(&state, reason.as_str());
                tracing::info!(reason, "Received internal drain request, shutting down gateway worker gracefully...");
            }
        }
    }

    #[cfg(not(unix))]
    {
        let ctrl_c = tokio::signal::ctrl_c();
        let internal_shutdown = state.shutdown.wait();
        tokio::select! {
            _ = ctrl_c => {
                mark_runtime_draining(&state, "shutdown_signal");
                tracing::info!("Received shutdown signal, shutting down gateway worker gracefully...");
            }
            _ = internal_shutdown => {
                let reason = state.shutdown.reason().unwrap_or_else(|| "internal_shutdown".to_string());
                mark_runtime_draining(&state, reason.as_str());
                tracing::info!(reason, "Received internal drain request, shutting down gateway worker gracefully...");
            }
        }
    }
}

pub fn mark_runtime_draining(state: &Arc<AppState>, reason: &str) {
    let started = state.lifecycle.begin_drain(reason);
    tracing::info!(
        reason,
        started,
        active_requests = state.lifecycle.active_requests(),
        "Gateway runtime marked as draining"
    );
}

pub async fn load_route_config_from_path(
    pool: &deadpool_redis::Pool,
    yaml_path: &std::path::Path,
    redis_namespace: &str,
) -> RouteConfigStore {
    match RouteConfigRedisStore::new(redis_namespace) {
        Ok(redis_store) => match redis_store.load_active_store(pool).await {
            Ok(Some(store)) => {
                tracing::info!(
                    providers = store.provider_count(),
                    namespace = redis_namespace,
                    "Loaded route config from console Redis revision state"
                );
                return store;
            }
            Ok(None) => {}
            Err(error) => {
                tracing::debug!(
                    code = error.code(),
                    namespace = redis_namespace,
                    "Console Redis route config not available: {}",
                    error
                );
            }
        },
        Err(error) => {
            tracing::warn!(
                code = error.code(),
                namespace = redis_namespace,
                "Console Redis namespace is invalid: {}",
                error
            );
        }
    }

    if redis_namespace == "default" {
        match RouteConfigStore::load_from_redis(pool).await {
            Ok(store) => {
                tracing::info!(
                    providers = store.provider_count(),
                    "Loaded route config from legacy Redis mirror"
                );
                return store;
            }
            Err(error) => {
                tracing::debug!("Legacy Redis route config not available: {}", error);
            }
        }
    }

    match RouteConfigStore::load_from_yaml(yaml_path) {
        Ok(store) => {
            tracing::info!(
                providers = store.provider_count(),
                path = %yaml_path.display(),
                "Loaded route config from YAML"
            );
            return store;
        }
        Err(error) => {
            tracing::warn!(
                "YAML route config not available ({}): {}",
                yaml_path.display(),
                error
            );
        }
    }

    tracing::warn!("No route config found — gateway will reject all model requests");
    RouteConfigStore::new()
}
