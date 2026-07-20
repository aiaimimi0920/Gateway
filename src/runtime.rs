use std::sync::Arc;

use crate::auth::adapter::AuthAdapter;
use crate::auth::project_api_key::ProjectApiKeyAdapter;
use crate::auth::user_credential::UserCredentialAdapter;
use crate::concurrency::aimd::AimdConfig;
use crate::concurrency::registry::ConcurrencyRegistry;
use crate::config::Config;
use crate::credential_store::CredentialMemoryCache;
use crate::db::create_pg_pool;
use crate::redis::pool::create_pool;
use crate::routing::config::RouteConfigStore;
use crate::state::{
    AppState, GatewayLifecycleState, GatewayShutdownHandle, ProviderCredentialFolderSyncRuntime,
};
use crate::upstream::client::UpstreamClient;

pub async fn run_gateway_runtime(config: Config) -> anyhow::Result<()> {
    let port = config.port;
    tracing::info!(port, role = ?config.runtime_role, "Starting neuro-gateway worker runtime");

    if config.gateway_api_key.is_some() {
        tracing::info!("Gateway API key fallback authentication enabled");
    } else {
        tracing::info!("No GATEWAY_API_KEY set — relying on configured auth adapters");
    }

    let app_state = build_app_state(config).await?;
    spawn_background_tasks(&app_state);
    run_gateway_server(app_state, port).await
}

pub async fn build_app_state(config: Config) -> anyhow::Result<Arc<AppState>> {
    let redis_pool = create_pool(&config.redis_url)?;
    let provider_credential_folder_sync_enabled =
        crate::provider_credential_folder_sync::load_runtime_enabled(&redis_pool, &config)
            .await
            .unwrap_or_else(|error| {
                tracing::warn!(
                    error = %error.message,
                    "failed to load provider credential folder sync runtime setting; falling back to config"
                );
                crate::provider_credential_folder_sync::default_runtime_enabled(&config)
            });
    let pg_pool = match config.database_url.as_deref() {
        Some(database_url) => {
            let pool = create_pg_pool(database_url).await?;
            tracing::info!("PostgreSQL connectivity enabled for Rust-owned gateway state");
            Some(pool)
        }
        None => {
            tracing::warn!(
                "No GATEWAY_DATABASE_URL or DATABASE_URL configured; DB-owned gateway state disabled"
            );
            None
        }
    };

    let upstream_client = UpstreamClient::new_with_runtime(
        config.upstream_timeout_secs,
        Some(redis_pool.clone()),
        pg_pool.clone(),
    );
    let concurrency_registry = ConcurrencyRegistry::new(AimdConfig::default());
    let mut auth_adapters: Vec<Box<dyn AuthAdapter>> = Vec::new();
    if let (Some(pool), Some(secret)) = (pg_pool.clone(), config.gateway_api_key_secret.clone()) {
        auth_adapters.push(Box::new(ProjectApiKeyAdapter::new(pool, secret)));
    }
    auth_adapters.push(Box::new(UserCredentialAdapter::new(
        redis_pool.clone(),
        pg_pool.clone(),
    )));

    let route_config = load_route_config(&redis_pool).await;
    let credential_cache = CredentialMemoryCache::new(30);

    Ok(Arc::new(AppState {
        config,
        redis_pool,
        pg_pool,
        upstream_client,
        concurrency_registry,
        auth_adapters,
        filter_config: None,
        route_config: Arc::new(route_config),
        credential_cache,
        lifecycle: GatewayLifecycleState::default(),
        shutdown: GatewayShutdownHandle::default(),
        provider_credential_folder_sync: ProviderCredentialFolderSyncRuntime::new(
            provider_credential_folder_sync_enabled,
        ),
    }))
}

pub fn spawn_background_tasks(app_state: &Arc<AppState>) {
    let cache_clone = app_state.credential_cache.clone();
    let pool_clone = app_state.redis_pool.clone();
    tokio::spawn(async move {
        crate::credential_store::start_credential_refresh_task(cache_clone, pool_clone, 10).await;
    });

    let rc_for_refresh = app_state.route_config.clone();
    let http_for_refresh = app_state.upstream_client.client().clone();
    let pool_for_refresh = app_state.redis_pool.clone();
    tokio::spawn(async move {
        crate::token_refresh::start_token_refresh_task(
            rc_for_refresh,
            http_for_refresh,
            pool_for_refresh,
            60,
            300,
        )
        .await;
    });

    let state_for_folder_sync = Arc::clone(app_state);
    tokio::spawn(async move {
        crate::provider_credential_folder_sync::start_folder_sync_task(state_for_folder_sync).await;
    });

    let state_for_credential_refresh = Arc::clone(app_state);
    tokio::spawn(async move {
        crate::provider_credential_refresh::start_provider_credential_refresh_task(
            state_for_credential_refresh,
        )
        .await;
    });

    let state_for_credential_stock = Arc::clone(app_state);
    tokio::spawn(async move {
        crate::credential_stock::start_credential_stock_monitor_task(state_for_credential_stock)
            .await;
    });
}

pub async fn run_gateway_server(app_state: Arc<AppState>, port: u16) -> anyhow::Result<()> {
    let app = crate::http::router::build_router(Arc::clone(&app_state));
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}")).await?;
    tracing::info!(port, "Gateway worker listening");

    let shutdown_state = Arc::clone(&app_state);
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal(shutdown_state))
        .await?;

    tracing::info!(port, "Gateway worker shut down complete");
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

pub async fn load_route_config(pool: &deadpool_redis::Pool) -> RouteConfigStore {
    match RouteConfigStore::load_from_redis(pool).await {
        Ok(store) => {
            tracing::info!(
                providers = store.provider_count(),
                "Loaded route config from Redis"
            );
            return store;
        }
        Err(error) => {
            tracing::debug!("Redis route config not available: {}", error);
        }
    }

    let yaml_path =
        std::env::var("GATEWAY_ROUTES_FILE").unwrap_or_else(|_| "routes.yaml".to_string());
    match RouteConfigStore::load_from_yaml(&yaml_path) {
        Ok(store) => {
            tracing::info!(
                providers = store.provider_count(),
                path = %yaml_path,
                "Loaded route config from YAML"
            );
            return store;
        }
        Err(error) => {
            tracing::warn!("YAML route config not available ({}): {}", yaml_path, error);
        }
    }

    tracing::warn!("No route config found — gateway will reject all model requests");
    RouteConfigStore::new()
}
