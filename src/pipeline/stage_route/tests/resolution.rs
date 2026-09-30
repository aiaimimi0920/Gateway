use super::*;

#[tokio::test]
async fn route_stage_errors_when_no_candidates() {
    let state = make_state();
    let mut ctx = PipelineContext::new(make_request(), None);
    // With an empty RouteConfigStore and no Redis credentials → should error.
    let result = run(&mut ctx, &state).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn route_stage_succeeds_with_configured_routes() {
    // Write a temp YAML config file for the test.
    let yaml = r#"
providers:
  - id: openai-default
    preset: openai
    base_url: "https://api.openai.com"
    api_key: "sk-test"
model_routes:
  - pattern: "gpt-*"
    provider_ids: [openai-default]
    priority: 10
"#;
    let tmp_dir = std::env::temp_dir();
    let tmp_path = tmp_dir.join("gw_test_routes_dual.yaml");
    std::fs::write(&tmp_path, yaml).unwrap();

    let store = RouteConfigStore::load_from_yaml(&tmp_path).unwrap();
    let state = Arc::new(AppState {
        config: make_config(),
        redis_pool: deadpool_redis::Config::from_url(test_redis_url())
            .create_pool(Some(deadpool_redis::Runtime::Tokio1))
            .expect("pool"),
        pg_pool: None,
        local_runtime: None,
        upstream_client: UpstreamClient::new(30),
        concurrency_registry: ConcurrencyRegistry::new(AimdConfig::default()),
        auth_adapters: vec![],
        filter_config: None,
        route_config: Arc::new(store),
        route_config_runtime: None,
        console_auth: test_console_auth_runtime(),
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

    let mut ctx = PipelineContext::new(make_request(), None);
    let result = run(&mut ctx, &state).await;
    assert!(
        result.is_ok(),
        "stage_route should succeed: {:?}",
        result.err()
    );
    assert!(
        !ctx.candidates.is_empty(),
        "should have at least one candidate"
    );
    assert_eq!(ctx.candidates[0].provider_account_id, "openai-default");

    let _ = std::fs::remove_file(&tmp_path);
}

#[tokio::test]
async fn dual_resolution_falls_back_to_yaml_when_redis_empty() {
    // YAML provides routes; Redis has no credentials.
    // The stage should still resolve from YAML successfully.
    let yaml = r#"
providers:
  - id: anthropic-fallback
    preset: anthropic
    base_url: "https://api.anthropic.com"
    api_key: "sk-ant-test"
model_routes:
  - pattern: "claude-*"
    provider_ids: [anthropic-fallback]
    priority: 10
"#;
    let tmp_dir = std::env::temp_dir();
    let tmp_path = tmp_dir.join("gw_test_dual_fallback.yaml");
    std::fs::write(&tmp_path, yaml).unwrap();

    let store = RouteConfigStore::load_from_yaml(&tmp_path).unwrap();
    let state = Arc::new(AppState {
        config: make_config(),
        redis_pool: deadpool_redis::Config::from_url(test_redis_url())
            .create_pool(Some(deadpool_redis::Runtime::Tokio1))
            .expect("pool"),
        pg_pool: None,
        local_runtime: None,
        upstream_client: UpstreamClient::new(30),
        concurrency_registry: ConcurrencyRegistry::new(AimdConfig::default()),
        auth_adapters: vec![],
        filter_config: None,
        route_config: Arc::new(store),
        route_config_runtime: None,
        console_auth: test_console_auth_runtime(),
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

    // Request for claude model — YAML should serve it
    let mut req = make_request();
    req.requested_model = Some("claude-sonnet-4-6".to_string());
    let mut ctx = PipelineContext::new(req, None);

    let result = run(&mut ctx, &state).await;
    assert!(
        result.is_ok(),
        "should fall back to YAML: {:?}",
        result.err()
    );
    assert_eq!(ctx.candidates[0].provider_account_id, "anthropic-fallback");

    let _ = std::fs::remove_file(&tmp_path);
}
