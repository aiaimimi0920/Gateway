use super::*;
use crate::concurrency::aimd::AimdConfig;
use crate::concurrency::registry::ConcurrencyRegistry;
use crate::config::Config;
use crate::console::ConsoleConfigValues;
use crate::routing::config::{RouteConfigStore, RouteConfigYaml};
use crate::state::{
    GatewayLifecycleState, GatewayShutdownHandle, ProviderCredentialFolderSyncRuntime,
};
use crate::upstream::client::UpstreamClient;
use axum::response::IntoResponse;
use http_body_util::BodyExt;
use serde_json::Value;

fn make_config() -> Config {
    Config {
        console: Default::default(),
        runtime_role: crate::config::GatewayRuntimeRole::Standalone,
        storage_mode: Default::default(),
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
        gateway_management_token: Some("management-token".to_string()),
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
    }
}

fn test_console_auth_runtime(
    env_management_token: Option<&str>,
) -> Arc<crate::console::ConsoleAuthRuntime> {
    let temp = std::env::temp_dir().join(format!(
        "gateway-account-groups-console-{}",
        uuid::Uuid::new_v4()
    ));
    let console = crate::console::ConsoleConfig::from_values(ConsoleConfigValues {
        state_dir: Some(temp.clone()),
        routes_file: Some(temp.join("routes.yaml")),
        ..Default::default()
    })
    .unwrap();
    Arc::new(
        crate::console::ConsoleAuthRuntime::new(&console, env_management_token.map(str::to_string))
            .unwrap(),
    )
}

fn make_state_with_console_auth(
    route_config: RouteConfigStore,
    console_auth: Arc<crate::console::ConsoleAuthRuntime>,
) -> Arc<AppState> {
    Arc::new(AppState {
        config: make_config(),
        redis_pool: deadpool_redis::Config::from_url("redis://localhost:6379")
            .create_pool(Some(deadpool_redis::Runtime::Tokio1))
            .expect("pool"),
        pg_pool: None,
        local_runtime: None,
        upstream_client: UpstreamClient::new(30),
        concurrency_registry: ConcurrencyRegistry::new(AimdConfig::default()),
        auth_adapters: vec![],
        filter_config: None,
        route_config: Arc::new(route_config),
        route_config_runtime: None,
        console_auth,
        credential_cache: crate::credential_store::CredentialMemoryCache::new(30),
        lifecycle: GatewayLifecycleState::default(),
        shutdown: GatewayShutdownHandle::default(),
        provider_credential_folder_sync: ProviderCredentialFolderSyncRuntime::new(false),
        credential_pool_automation: Arc::new(
            crate::credential_pool_automation::CredentialPoolAutomationRuntime::disabled(),
        ),
    })
}

fn make_state(route_config: RouteConfigStore) -> Arc<AppState> {
    make_state_with_console_auth(
        route_config,
        test_console_auth_runtime(Some("management-token")),
    )
}

fn load_legacy_route_config(yaml: &str) -> RouteConfigStore {
    let path = std::env::temp_dir().join(format!(
        "gateway-account-groups-legacy-{}.yaml",
        uuid::Uuid::new_v4()
    ));
    std::fs::write(&path, yaml).expect("write legacy route config");
    let route_config = RouteConfigStore::load_from_yaml(&path);
    let _ = std::fs::remove_file(&path);
    route_config.expect("load legacy route config")
}

fn sensitive_url_state() -> Arc<AppState> {
    let route_config = load_legacy_route_config(
        r#"
providers:
  - id: sensitive-provider
adapter: openai_compatible
base_url: "https://provider-user:provider-password@example.com/v1?api_key=provider-query-secret&safe=provider#provider-fragment"
api_key: provider-api-secret
credentials:
  - id: sensitive-credential
    base_url: "https://credential-user:credential-password@example.com/v1?token=credential-query-secret&safe=credential#credential-fragment"
    api_key: credential-api-secret
model_routes: []
aliases: {}
"#,
    );
    make_state(route_config)
}

async fn sensitive_url_summary_response() -> axum::response::Response {
    let mut headers = HeaderMap::new();
    headers.insert("x-internal-api-key", "management-token".parse().unwrap());
    get_account_groups_summary(
        State(sensitive_url_state()),
        None,
        OptionalBearerToken(None),
        headers,
    )
    .await
    .expect("summary response")
    .into_response()
}

#[tokio::test]
async fn account_group_summary_redacts_sensitive_url_components() {
    let response = sensitive_url_summary_response().await;
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("summary body")
        .to_bytes();
    let payload: Value = serde_json::from_slice(&bytes).expect("summary JSON");
    let serialized = serde_json::to_string(&payload).unwrap();
    for secret in [
        "provider-user",
        "provider-password",
        "provider-query-secret",
        "provider-fragment",
        "credential-user",
        "credential-password",
        "credential-query-secret",
        "credential-fragment",
    ] {
        assert!(!serialized.contains(secret), "summary leaked {secret}");
    }

    let provider_url = payload["summary"]["providers"][0]["baseUrl"]
        .as_str()
        .expect("provider base URL");
    let account_url = payload["summary"]["accounts"][0]["baseUrl"]
        .as_str()
        .expect("account base URL");
    assert_eq!(provider_url, "https://example.com/v1?safe=provider");
    assert_eq!(account_url, "https://example.com/v1?safe=credential");
}

#[tokio::test]
async fn account_group_summary_disables_response_caching() {
    let response = sensitive_url_summary_response().await;
    assert_eq!(
        response
            .headers()
            .get("cache-control")
            .and_then(|value| value.to_str().ok()),
        Some("no-store")
    );
}

#[tokio::test]
async fn account_group_summary_accepts_bootstrapped_console_management_token() {
    let document: RouteConfigYaml = serde_yaml::from_str(
        r#"
providers:
  - id: bootstrapped-provider
base_url: https://example.com/v1
api_key: provider-secret
model_routes: []
aliases: {}
"#,
    )
    .unwrap();
    let console_auth = test_console_auth_runtime(None);
    console_auth
        .bootstrap(
            &crate::console::ConsoleRequestContext::new(
                "127.0.0.1".parse().unwrap(),
                "local".to_string(),
            ),
            "bootstrapped-token",
        )
        .unwrap();
    let state = make_state_with_console_auth(
        RouteConfigStore::from_document(document).unwrap(),
        console_auth,
    );
    let mut headers = HeaderMap::new();
    headers.insert("x-management-token", "bootstrapped-token".parse().unwrap());

    let response =
        get_account_groups_summary(State(state), None, OptionalBearerToken(None), headers)
            .await
            .expect("bootstrapped console token should authenticate");
    assert_eq!(response.status(), axum::http::StatusCode::OK);
}

#[tokio::test]
async fn account_group_summary_exposes_effective_billing_multiplier_and_memberships() {
    let route_config = RouteConfigStore::from_document(RouteConfigYaml {
        providers: vec![
            crate::routing::config::ProviderConfigYaml {
                id: "openai-default".to_string(),
                label: Some("OpenAI".to_string()),
                vendor_key: None,
                vendor_name: None,
                preset: Some("openai".to_string()),
                base_url: "https://api.openai.com".to_string(),
                api_key: "sk-openai".to_string(),
                auth_token: None,
                credential_storage_password: None,
                credential_storage_path: None,
                credential_archive_path: None,
                headers: Default::default(),
                extra_body: Default::default(),
                session_auth: None,
                keepalive: None,
                expires_at: None,
                runtime_state_object_key: None,
                account_name: Some("OpenAI Shared".to_string()),
                execution_mode: None,
                endpoint_execution_modes: None,
                default_model: None,
                adapter: None,
                protocol_family: None,
                protocol_profile: None,
                supported_models: vec!["gpt-5.4".to_string()],
                responses_path: None,
                chat_completions_path: None,
                completions_path: None,
                embeddings_path: None,
                audio_transcriptions_path: None,
                audio_speech_path: None,
                messages_path: None,
                search_path: None,
                fetch_path: None,
                research_path: None,
                balance_path: None,
                search_query_field: None,
                fetch_urls_field: None,
                model_map: Default::default(),
                pool_target_size: None,
                pool_min_size: None,
                pool_refill_in_progress: false,
                auto_refill_enabled: false,
                auto_prune_enabled: false,
                scheduled_probe_enabled: false,
                scheduled_probe_interval_minutes: None,
                credential_permanent_delete_enabled: false,
                credential_automation_driver_id: None,
                credential_identity_categories: vec![],
                credentials: vec![],
            },
            crate::routing::config::ProviderConfigYaml {
                id: "codex-main".to_string(),
                label: Some("Codex".to_string()),
                vendor_key: None,
                vendor_name: None,
                preset: Some("codex".to_string()),
                base_url: "https://muyuan.do/v1".to_string(),
                api_key: "".to_string(),
                auth_token: None,
                credential_storage_password: None,
                credential_storage_path: None,
                credential_archive_path: None,
                headers: Default::default(),
                extra_body: Default::default(),
                session_auth: None,
                keepalive: None,
                expires_at: None,
                runtime_state_object_key: None,
                account_name: None,
                execution_mode: None,
                endpoint_execution_modes: None,
                default_model: None,
                adapter: None,
                protocol_family: None,
                protocol_profile: None,
                supported_models: vec!["gpt-5.4".to_string()],
                responses_path: None,
                chat_completions_path: None,
                completions_path: None,
                embeddings_path: None,
                audio_transcriptions_path: None,
                audio_speech_path: None,
                messages_path: None,
                search_path: None,
                fetch_path: None,
                research_path: None,
                balance_path: None,
                search_query_field: None,
                fetch_urls_field: None,
                model_map: Default::default(),
                pool_target_size: None,
                pool_min_size: None,
                pool_refill_in_progress: false,
                auto_refill_enabled: false,
                auto_prune_enabled: false,
                scheduled_probe_enabled: false,
                scheduled_probe_interval_minutes: None,
                credential_permanent_delete_enabled: false,
                credential_automation_driver_id: None,
                credential_identity_categories: vec![],
                credentials: vec![crate::routing::config::ProviderCredentialYaml {
                    id: Some("codex-live".to_string()),
                    base_url: None,
                    api_key: Some("sk-codex".to_string()),
                    auth_token: None,
                    headers: Default::default(),
                    extra_body: Default::default(),
                    session_auth: None,
                    keepalive: None,
                    expires_at: None,
                    runtime_state_object_key: None,
                    account_name: Some("Codex Live".to_string()),
                    credential_identity_category_id: None,
                    enabled: None,
                    scheduled_probe_enabled: false,
                    scheduled_probe_interval_minutes: None,
                    execution_mode: None,
                    endpoint_execution_modes: None,
                    supported_models: vec!["gpt-5.4".to_string()],
                    refresh_token: None,
                    refresh_endpoint: None,
                    refresh_client_id: None,
                    token_expires_in_secs: None,
                }],
            },
        ],
        model_routes: vec![],
        aliases: Default::default(),
        account_groups: vec![crate::routing::config::AccountGroupYaml {
            id: "premium".to_string(),
            name: "Premium".to_string(),
            description: Some("VIP accounts".to_string()),
            billing_multiplier: Some(1.25),
            enabled: Some(true),
            notes: Some("preferred".to_string()),
            provider_credential_ids: vec![
                "openai-default::default".to_string(),
                "codex-live".to_string(),
            ],
        }],
    })
    .expect("route config");
    let state = make_state(route_config);
    let mut headers = HeaderMap::new();
    headers.insert("x-internal-api-key", "management-token".parse().unwrap());

    let response =
        get_account_groups_summary(State(state), None, OptionalBearerToken(None), headers)
            .await
            .expect("summary response");
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("summary body")
        .to_bytes();
    let payload: Value = serde_json::from_slice(&bytes).expect("summary JSON");

    let summary = payload.get("summary").and_then(Value::as_object).unwrap();
    let groups = summary
        .get("accountGroups")
        .and_then(Value::as_array)
        .expect("group list");
    assert_eq!(groups.len(), 1);
    assert_eq!(
        groups[0]
            .get("billingMultiplier")
            .and_then(Value::as_f64)
            .unwrap(),
        1.25
    );
    assert_eq!(
        groups[0]
            .get("memberCount")
            .and_then(Value::as_u64)
            .unwrap(),
        2
    );

    let accounts = summary
        .get("accounts")
        .and_then(Value::as_array)
        .expect("account list");
    assert!(accounts.iter().any(|account| {
        account.get("id").and_then(Value::as_str) == Some("openai-default::default")
            && account
                .get("groupIds")
                .and_then(Value::as_array)
                .is_some_and(|group_ids| {
                    group_ids
                        .iter()
                        .any(|value| value.as_str() == Some("premium"))
                })
    }));
    assert!(accounts.iter().any(|account| {
        account.get("id").and_then(Value::as_str) == Some("codex-live")
            && account.get("displayName").and_then(Value::as_str) == Some("Codex Live")
    }));
}
