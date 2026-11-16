//! Isolated AppState with the same store Arc as its Worker owner and a synthetic cache pool.
use super::{authority_fixture::Authority, cache_wire::Cache, *};
use crate::{
    config::{Config, GatewayRuntimeRole, GatewayStorageMode},
    console::ConsoleAuthRuntime,
};

pub(super) fn state(authority: &Authority, cache: &Cache) -> Arc<AppState> {
    let root = authority.routes.parent().unwrap();
    let console = ConsoleConfig::from_values(ConsoleConfigValues {
        state_dir: Some(root.join("auth-state")),
        routes_file: Some(authority.routes.clone()),
        ..Default::default()
    })
    .unwrap();
    let config = Config {
        console,
        runtime_role: GatewayRuntimeRole::Worker,
        storage_mode: GatewayStorageMode::Server,
        port: 0,
        redis_url: "redis://127.0.0.1:1".into(), // The injected pool below is the only actual cache destination.
        database_url: None,
        upstream_timeout_secs: 3,
        max_request_body_bytes: 1024,
        max_body_chat_completions_bytes: 1024,
        max_body_completions_bytes: 1024,
        max_body_messages_bytes: 1024,
        max_body_responses_bytes: 1024,
        max_body_embeddings_bytes: 1024,
        max_body_audio_transcriptions_bytes: 1024,
        max_body_audio_speech_bytes: 1024,
        max_body_search_bytes: 1024,
        max_body_fetch_bytes: 1024,
        max_body_research_bytes: 1024,
        max_body_images_generations_bytes: 1024,
        max_body_images_edits_bytes: 1024,
        max_body_music_bytes: 1024,
        max_body_videos_bytes: 1024,
        response_cache_ttl_secs: 1,
        response_cache_max_size_bytes: 1024,
        quota_pre_deduct_estimate_ratio: 1.2,
        usage_report_batch_size: 1,
        provider_probe_interval_secs: 1,
        log_level: "error".into(),
        gateway_api_key: None,
        gateway_api_key_secret: None,
        gateway_management_token: None,
        gateway_keepalive_bearer_token: None,
        default_project_id: "synthetic-project".into(),
        gateway_inbound_api_key_header_aliases: Vec::new(),
        provider_credential_folder_sync_enabled: false,
        provider_credential_folder_sync_root_dir: None,
        provider_credential_folder_sync_interval_secs: 30,
        provider_credential_folder_sync_watch_enabled: false,
        provider_credential_folder_sync_watch_debounce_millis: 100,
        provider_credential_folder_sync_import_enabled: false,
        provider_credential_folder_sync_export_enabled: false,
        provider_credential_folder_sync_delete_missing: false,
        provider_credential_refresh_enabled: false,
        provider_credential_refresh_interval_secs: 3600,
        provider_credential_refresh_before_secs: 300,
        provider_credential_refresh_batch_limit: 1,
        provider_credential_refresh_lock_ttl_secs: 300,
        credential_stock_monitor_enabled: false,
        credential_stock_monitor_interval_secs: 60,
        credential_pool_automation: Default::default(),
        splitter_worker_executable_path: None,
        splitter_initial_worker_port: 1,
        splitter_ready_timeout_secs: 1,
        splitter_ready_poll_interval_millis: 10,
        splitter_reload_shutdown_timeout_secs: 1,
    };
    let console_auth = Arc::new(ConsoleAuthRuntime::new(&config.console, None).unwrap());
    Arc::new(AppState {
        config,
        redis_pool: cache.pool.clone(),
        pg_pool: None,
        local_runtime: None,
        upstream_client: crate::upstream::client::UpstreamClient::new(3),
        concurrency_registry: crate::concurrency::registry::ConcurrencyRegistry::new(
            Default::default(),
        ),
        routing_latency: Default::default(),
        auth_adapters: Vec::new(),
        filter_config: None,
        route_config: authority.runtime.route_config().clone(),
        route_config_runtime: Some(Arc::new(authority.runtime.clone())),
        console_auth,
        credential_cache: crate::credential_store::CredentialMemoryCache::new(30),
        lifecycle: Default::default(),
        shutdown: Default::default(),
        provider_credential_folder_sync: crate::state::ProviderCredentialFolderSyncRuntime::new(
            false,
        ),
        credential_pool_automation: Arc::new(
            crate::credential_pool_automation::CredentialPoolAutomationRuntime::disabled(),
        ),
    })
}
