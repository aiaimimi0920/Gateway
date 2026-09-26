use super::*;

#[tokio::test]
async fn empty_candidates_returns_error() {
    use crate::concurrency::aimd::AimdConfig;
    use crate::concurrency::registry::ConcurrencyRegistry;
    use crate::config::Config;
    use crate::console::{ConsoleAuthRuntime, ConsoleConfig, ConsoleConfigValues};
    use crate::pipeline::PipelineContext;
    use crate::protocol::canonical::{CanonicalMessage, ContentPart, MessageRole};
    use crate::upstream::client::UpstreamClient;

    let console_temp = std::env::temp_dir().join(format!(
        "gateway-stage-send-console-{}",
        uuid::Uuid::new_v4()
    ));
    let console = ConsoleConfig::from_values(ConsoleConfigValues {
        state_dir: Some(console_temp.clone()),
        routes_file: Some(console_temp.join("routes.yaml")),
        ..Default::default()
    })
    .unwrap();
    let state = Arc::new(AppState {
        config: Config {
            console: console.clone(),
            runtime_role: crate::config::GatewayRuntimeRole::Standalone,
            port: 4200,
            redis_url: "redis://localhost".to_string(),
            database_url: None,
            upstream_timeout_secs: 30,
            max_request_body_bytes: 1024 * 1024,
            max_body_chat_completions_bytes: 1024 * 1024,
            max_body_completions_bytes: 1024 * 1024,
            max_body_messages_bytes: 1024 * 1024,
            max_body_responses_bytes: 1024 * 1024,
            max_body_embeddings_bytes: 1024 * 1024,
            max_body_audio_transcriptions_bytes: 8 * 1024 * 1024,
            max_body_audio_speech_bytes: 1024 * 1024,
            max_body_search_bytes: 512 * 1024,
            max_body_fetch_bytes: 512 * 1024,
            max_body_research_bytes: 512 * 1024,
            max_body_images_generations_bytes: 1024 * 1024,
            max_body_images_edits_bytes: 1024 * 1024,
            max_body_music_bytes: 1024 * 1024,
            max_body_videos_bytes: 1024 * 1024,
            response_cache_ttl_secs: 300,
            response_cache_max_size_bytes: 512 * 1024,
            quota_pre_deduct_estimate_ratio: 1.2,
            usage_report_batch_size: 100,
            provider_probe_interval_secs: 30,
            log_level: "info".to_string(),
            gateway_api_key: None,
            gateway_api_key_secret: None,
            gateway_management_token: None,
            gateway_keepalive_bearer_token: None,
            default_project_id: "platform-default-project".to_string(),
            gateway_inbound_api_key_header_aliases: Vec::new(),
            provider_credential_folder_sync_enabled: false,
            provider_credential_folder_sync_root_dir: None,
            provider_credential_folder_sync_interval_secs: 30,
            provider_credential_folder_sync_import_enabled: true,
            provider_credential_folder_sync_export_enabled: true,
            provider_credential_folder_sync_watch_enabled: true,
            provider_credential_folder_sync_watch_debounce_millis: 1500,
            provider_credential_folder_sync_delete_missing: false,
            provider_credential_refresh_enabled: true,
            provider_credential_refresh_interval_secs: 3600,
            provider_credential_refresh_before_secs: 86_400,
            provider_credential_refresh_batch_limit: 100,
            provider_credential_refresh_lock_ttl_secs: 300,
            credential_stock_monitor_enabled: true,
            credential_stock_monitor_interval_secs: 60,
            credential_pool_automation: Default::default(),
            splitter_worker_executable_path: None,
            splitter_initial_worker_port: 4201,
            splitter_ready_timeout_secs: 120,
            splitter_ready_poll_interval_millis: 500,
            splitter_reload_shutdown_timeout_secs: 600,
        },
        redis_pool: deadpool_redis::Config::from_url("redis://localhost:6379")
            .create_pool(Some(deadpool_redis::Runtime::Tokio1))
            .expect("pool"),
        pg_pool: None,
        upstream_client: UpstreamClient::new(30),
        concurrency_registry: ConcurrencyRegistry::new(AimdConfig::default()),
        auth_adapters: vec![],
        filter_config: None,
        route_config: Arc::new(crate::routing::config::RouteConfigStore::new()),
        route_config_runtime: None,
        console_auth: Arc::new(ConsoleAuthRuntime::new(&console, None).unwrap()),
        credential_cache: crate::credential_store::CredentialMemoryCache::new(30),
        lifecycle: crate::state::GatewayLifecycleState::default(),
        shutdown: crate::state::GatewayShutdownHandle::default(),
        provider_credential_folder_sync: crate::state::ProviderCredentialFolderSyncRuntime::new(
            false,
        ),
        credential_pool_automation: Arc::new(
            crate::credential_pool_automation::CredentialPoolAutomationRuntime::disabled(),
        ),
    });

    let req = CanonicalRelayRequest {
        protocol_family: ProtocolFamily::OpenAi,
        endpoint_kind: EndpointKind::ChatCompletions,
        requested_model: Some("gpt-4o".to_string()),
        stream: false,
        messages: vec![CanonicalMessage {
            role: MessageRole::User,
            content: vec![ContentPart::Text {
                text: "hi".to_string(),
            }],
            name: None,
            tool_call_id: None,
            tool_calls: vec![],
        }],
        tools: vec![],
        tool_choice: None,
        reasoning: None,
        metadata: None,
        raw_body: serde_json::json!({}),
        previous_response_id: None,
        explicit_session_key: None,
        extra: HashMap::new(),
    };

    let mut ctx = PipelineContext::new(req, None);
    // candidates is empty by default
    let result = run(&mut ctx, &state).await;
    assert!(result.is_err());
}
