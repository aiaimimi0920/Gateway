use super::*;

#[tokio::test]
async fn route_stage_filters_yaml_candidates_by_requested_account_group() {
    let yaml = r#"
providers:
  - id: openai-default
    preset: openai
    base_url: "https://api.openai.com"
    api_key: "sk-openai"
    supported_models: [gpt-5.4]
  - id: codex-main
    preset: codex
    base_url: "https://muyuan.do/v1"
    api_key: ""
    supported_models: [gpt-5.4]
    credentials:
      - id: codex-live
        api_key: "sk-codex"
      - id: codex-spare
        api_key: "sk-codex-2"
account_groups:
  - id: premium
    name: "Premium"
    provider_credential_ids: [codex-live]
model_routes:
  - pattern: "gpt-*"
    provider_ids: [openai-default, codex-main]
    priority: 10
"#;
    let tmp_dir = std::env::temp_dir();
    let tmp_path = tmp_dir.join("gw_test_account_group_filter.yaml");
    std::fs::write(&tmp_path, yaml).unwrap();

    let store = RouteConfigStore::load_from_yaml(&tmp_path).unwrap();
    let state = Arc::new(AppState {
        config: make_config(),
        redis_pool: deadpool_redis::Config::from_url(test_redis_url())
            .create_pool(Some(deadpool_redis::Runtime::Tokio1))
            .expect("pool"),
        pg_pool: None,
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

    let mut req = make_request();
    req.requested_model = Some("gpt-5.4".to_string());
    let mut ctx = PipelineContext::new(req, None);
    ctx.account_group_id = Some("premium".to_string());

    let result = run(&mut ctx, &state).await;
    assert!(
        result.is_ok(),
        "grouped route should succeed: {:?}",
        result.err()
    );
    assert_eq!(ctx.candidates.len(), 1);
    assert_eq!(ctx.candidates[0].provider_account_id, "codex-main");
    assert_eq!(
        ctx.candidates[0].payload.credential_id.as_deref(),
        Some("codex-live")
    );

    let _ = std::fs::remove_file(&tmp_path);
}

#[tokio::test]
async fn route_stage_errors_for_unknown_requested_account_group() {
    let yaml = r#"
providers:
  - id: openai-default
    preset: openai
    base_url: "https://api.openai.com"
    api_key: "sk-openai"
    supported_models: [gpt-5.4]
model_routes:
  - pattern: "gpt-*"
    provider_ids: [openai-default]
    priority: 10
"#;
    let tmp_dir = std::env::temp_dir();
    let tmp_path = tmp_dir.join("gw_test_account_group_missing.yaml");
    std::fs::write(&tmp_path, yaml).unwrap();

    let store = RouteConfigStore::load_from_yaml(&tmp_path).unwrap();
    let state = Arc::new(AppState {
        config: make_config(),
        redis_pool: deadpool_redis::Config::from_url(test_redis_url())
            .create_pool(Some(deadpool_redis::Runtime::Tokio1))
            .expect("pool"),
        pg_pool: None,
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

    let mut req = make_request();
    req.requested_model = Some("gpt-5.4".to_string());
    let mut ctx = PipelineContext::new(req, None);
    ctx.account_group_id = Some("missing".to_string());

    let error = run(&mut ctx, &state)
        .await
        .expect_err("missing group should fail");
    assert_eq!(error.code.as_deref(), Some("account_group_not_found"));

    let _ = std::fs::remove_file(&tmp_path);
}

#[test]
fn access_catalog_candidate_pairs_are_group_filtered_without_losing_row_alignment() {
    let store = RouteConfigStore::from_document(
        serde_yaml::from_str(
            r#"
providers:
  - id: access-provider
    base_url: "https://example.com"
    credentials:
      - { id: access-live, api_key: live-key }
      - { id: access-outside, api_key: outside-key }
account_groups:
  - id: isolated
    name: Isolated
    provider_credential_ids: [access-live]
model_routes: []
"#,
        )
        .expect("route document"),
    )
    .expect("route store");
    let template = store
        .resolve_candidates(None)
        .into_iter()
        .next()
        .expect("candidate template");
    let mut outside = template.clone();
    outside.provider_credential_id = Some("access-outside".to_string());
    outside.payload.credential_id = Some("access-outside".to_string());
    let mut live = template;
    live.provider_credential_id = Some("access-live".to_string());
    live.payload.credential_id = Some("access-live".to_string());
    let constraint = store
        .account_group_constraint(Some("isolated"))
        .expect("account group");

    let (candidates, rows) = filter_candidate_pairs_by_account_group(
        &constraint,
        vec![outside, live],
        vec!["outside-row", "live-row"],
    );

    assert_eq!(candidates.len(), 1);
    assert_eq!(
        candidates[0].payload.credential_id.as_deref(),
        Some("access-live")
    );
    assert_eq!(rows, vec!["live-row"]);
}

#[tokio::test]
async fn route_stage_filters_cached_redis_candidates_before_queue_selection() {
    let store = RouteConfigStore::from_document(
        serde_yaml::from_str(
            r#"
providers:
  - id: yaml-provider
    base_url: "https://yaml.example.com"
    supported_models: [gpt-5.4]
    credentials:
      - { id: yaml-live, api_key: yaml-key }
account_groups:
  - id: isolated
    name: Isolated
    provider_credential_ids: [yaml-live]
model_routes:
  - pattern: gpt-5.4
    provider_ids: [yaml-provider]
    priority: 10
"#,
        )
        .expect("route document"),
    )
    .expect("route store");
    let state = make_state_with_route_store(store);
    state.credential_cache.put(
        "default",
        "gpt-5.4",
        vec![make_cached_credential("redis-outside")],
    );
    let mut request = make_request();
    request.requested_model = Some("gpt-5.4".to_string());
    let mut ctx = PipelineContext::new(request, None);
    ctx.account_group_id = Some("isolated".to_string());

    run(&mut ctx, &state).await.expect("grouped route");

    assert_eq!(ctx.candidates.len(), 1);
    assert_eq!(
        ctx.candidates[0].payload.credential_id.as_deref(),
        Some("yaml-live")
    );
}

#[tokio::test]
async fn route_stage_never_falls_back_to_an_out_of_group_cached_credential() {
    let store = RouteConfigStore::from_document(
        serde_yaml::from_str(
            r#"
providers:
  - id: yaml-provider
    base_url: "https://yaml.example.com"
    supported_models: [gpt-5.4]
    credentials:
      - id: yaml-disabled
        api_key: disabled-key
        enabled: false
account_groups:
  - id: isolated
    name: Isolated
    provider_credential_ids: [yaml-disabled]
model_routes:
  - pattern: gpt-5.4
    provider_ids: [yaml-provider]
    priority: 10
"#,
        )
        .expect("route document"),
    )
    .expect("route store");
    let state = make_state_with_route_store(store);
    state.credential_cache.put(
        "default",
        "gpt-5.4",
        vec![make_cached_credential("redis-outside")],
    );
    let mut request = make_request();
    request.requested_model = Some("gpt-5.4".to_string());
    let mut ctx = PipelineContext::new(request, None);
    ctx.account_group_id = Some("isolated".to_string());

    let error = run(&mut ctx, &state)
        .await
        .expect_err("out-of-group Redis credential must not be used");

    assert_eq!(
        error.code.as_deref(),
        Some("account_group_candidates_unavailable")
    );
}
