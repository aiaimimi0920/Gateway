#![allow(dead_code)]

use std::sync::Arc;

use neuro_gateway::concurrency::aimd::AimdConfig;
use neuro_gateway::concurrency::registry::ConcurrencyRegistry;
use neuro_gateway::config::Config;
use neuro_gateway::console::{ConsoleAuthRuntime, RouteConfigRuntime};
use neuro_gateway::credential_store::CredentialMemoryCache;
use neuro_gateway::routing::config::RouteConfigStore;
use neuro_gateway::state::{
    AppState, GatewayLifecycleState, GatewayShutdownHandle, ProviderCredentialFolderSyncRuntime,
};
use neuro_gateway::upstream::client::UpstreamClient;

pub fn build_test_app_state(
    config: Config,
    route_config: RouteConfigStore,
    route_config_runtime: Option<Arc<RouteConfigRuntime>>,
) -> Arc<AppState> {
    let redis_pool = deadpool_redis::Config::from_url(config.redis_url.clone())
        .create_pool(Some(deadpool_redis::Runtime::Tokio1))
        .expect("create lazy Redis test pool");
    let upstream_timeout_secs = config.upstream_timeout_secs;
    let provider_credential_folder_sync_enabled = config.provider_credential_folder_sync_enabled;
    let console_auth = Arc::new(
        ConsoleAuthRuntime::new(&config.console, config.gateway_management_token.clone())
            .expect("create test console auth runtime"),
    );

    Arc::new(AppState {
        config,
        redis_pool,
        pg_pool: None,
        local_runtime: None,
        upstream_client: UpstreamClient::new(upstream_timeout_secs),
        concurrency_registry: ConcurrencyRegistry::new(AimdConfig::default()),
        auth_adapters: Vec::new(),
        filter_config: None,
        route_config: Arc::new(route_config),
        route_config_runtime,
        console_auth,
        credential_cache: CredentialMemoryCache::new(30),
        lifecycle: GatewayLifecycleState::default(),
        shutdown: GatewayShutdownHandle::default(),
        provider_credential_folder_sync: ProviderCredentialFolderSyncRuntime::new(
            provider_credential_folder_sync_enabled,
        ),
        credential_pool_automation: Arc::new(
            neuro_gateway::credential_pool_automation::CredentialPoolAutomationRuntime::disabled(),
        ),
    })
}
