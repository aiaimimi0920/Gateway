use super::*;

fn collect_model_catalog_ids_for_tests(
    route_policy: Option<&GatewayRoutePolicyConfig>,
    alias_rows: Vec<GatewayModelAliasRow>,
    provider_rows: Vec<GatewayProviderRouteRow>,
    capability_rows: Vec<ProviderCapabilityModelRow>,
    provider_default_models: HashMap<String, Option<String>>,
) -> Vec<String> {
    super::catalog_selection::collect_catalog_ids(
        route_policy,
        &alias_rows,
        &provider_rows,
        &capability_rows,
        &provider_default_models,
        &|_, _, _| true,
    )
}

fn make_provider_row(id: &str, protocol_family: &str) -> GatewayProviderRouteRow {
    GatewayProviderRouteRow {
        id: id.to_string(),
        label: id.to_string(),
        adapter: "openai_compatible".to_string(),
        protocol_family: protocol_family.to_string(),
        protocol_profile: "openai".to_string(),
        cooldown_until: None,
        failure_count: 0,
        execution_mode: "direct_http".to_string(),
        endpoint_execution_modes: None,
        payload_inline: None,
        payload_object_key: None,
    }
}

#[test]
fn normalize_route_policy_defaults_match_rust_baseline() {
    let config = normalize_route_policy_config(GatewayRoutePolicyConfigInput::default()).unwrap();
    assert_eq!(config.rate_limit_enforcement_version, None);
    assert_eq!(config.selection_strategy, "weighted_random");
    assert_eq!(config.circuit_breaker_threshold, 3);
    assert_eq!(
        config.fallback_http_statuses,
        vec![408, 425, 429, 500, 502, 503, 504]
    );
}

#[test]
fn malformed_route_policy_config_is_rejected_instead_of_defaulted() {
    let error = parse_route_policy_config(&serde_json::json!({
        "rateLimitEnforcementVersion": ["v1"]
    }))
    .expect_err("malformed persisted route policy must not silently use defaults");

    assert_eq!(error.code.as_deref(), Some("route_policy_config_invalid"));
}

#[test]
fn route_policy_blocks_disallowed_model() {
    let config = GatewayRoutePolicyConfig {
        allowed_model_ids: Some(vec!["gpt-4o".to_string()]),
        ..GatewayRoutePolicyConfig::default()
    };
    assert!(route_policy_allows_models(
        Some(&config),
        [Some("gpt-4o"), None]
    ));
    assert!(!route_policy_allows_models(
        Some(&config),
        [Some("claude-3-7"), None]
    ));
}

#[test]
fn route_policy_filters_provider_and_protocol() {
    let row = GatewayProviderRouteRow {
        id: "provider-a".to_string(),
        label: "Provider A".to_string(),
        adapter: "openai_compatible".to_string(),
        protocol_family: "openai".to_string(),
        protocol_profile: "openai".to_string(),
        cooldown_until: None,
        failure_count: 0,
        execution_mode: "direct_http".to_string(),
        endpoint_execution_modes: None,
        payload_inline: None,
        payload_object_key: None,
    };
    let config = GatewayRoutePolicyConfig {
        allowed_provider_account_ids: Some(vec!["provider-a".to_string()]),
        allowed_protocol_families: Some(vec!["openai".to_string()]),
        ..GatewayRoutePolicyConfig::default()
    };
    assert!(provider_allowed_by_route_policy(&row, Some(&config)));
}

#[test]
fn credential_model_allow_list_matches_alias_or_upstream_model() {
    let payload = serde_json::json!({
        "supportedModels": ["qwen3.5-35b-a3b", "astron-code-latest"]
    });
    assert!(credential_payload_supports_model(
        &payload,
        Some("qwen3.5-35b-a3b"),
        Some("astron-code-latest"),
    ));
    assert!(!credential_payload_supports_model(
        &payload,
        Some("qwen3.5-2b"),
        Some("xop35qwen2b"),
    ));
}

#[test]
fn credential_model_block_list_overrides_allow_list() {
    let payload = serde_json::json!({
        "supportedModels": ["qwen3.5-35b-a3b"],
        "excludedModels": ["astron-code-latest"]
    });
    assert!(!credential_payload_supports_model(
        &payload,
        Some("qwen3.5-35b-a3b"),
        Some("astron-code-latest"),
    ));
}

#[test]
fn model_catalog_prefers_platform_model_code_over_upstream_default_model() {
    let alias_rows = vec![GatewayModelAliasRow {
        id: "alias-1".to_string(),
        project_id: Some("project-1".to_string()),
        scope_type: "project".to_string(),
        alias: "qwen3.5-2b-xfyun-ws".to_string(),
        provider_account_id: "provider-xfyun-ws".to_string(),
        upstream_model: Some("xop35qwen2b".to_string()),
        priority: 0,
        weight: 1,
        enabled: true,
        created_at: OffsetDateTime::now_utc(),
        updated_at: OffsetDateTime::now_utc(),
    }];
    let capability_rows = vec![ProviderCapabilityModelRow {
        provider_account_id: "provider-xfyun-ws".to_string(),
        model_code: "qwen3.5-2b-xfyun-ws".to_string(),
        upstream_model: Some("xop35qwen2b".to_string()),
    }];
    let provider_rows = vec![make_provider_row("provider-xfyun-ws", "xfyun_websocket")];
    let default_models = HashMap::from([(
        "provider-xfyun-ws".to_string(),
        Some("xop35qwen2b".to_string()),
    )]);

    let model_ids = collect_model_catalog_ids_for_tests(
        None,
        alias_rows,
        provider_rows,
        capability_rows,
        default_models,
    );

    assert!(model_ids.contains(&"qwen3.5-2b-xfyun-ws".to_string()));
    assert!(!model_ids.contains(&"xop35qwen2b".to_string()));
}

#[test]
fn model_catalog_hides_upstream_capability_id_when_platform_alias_exists() {
    let alias_rows = vec![GatewayModelAliasRow {
        id: "alias-1".to_string(),
        project_id: Some("project-1".to_string()),
        scope_type: "project".to_string(),
        alias: "nvidia-llama-3-3-nemotron-super-49b-v1-5".to_string(),
        provider_account_id: "provider-nvidia".to_string(),
        upstream_model: Some("nvidia/llama-3.3-nemotron-super-49b-v1.5".to_string()),
        priority: 0,
        weight: 1,
        enabled: true,
        created_at: OffsetDateTime::now_utc(),
        updated_at: OffsetDateTime::now_utc(),
    }];
    let capability_rows = vec![
        ProviderCapabilityModelRow {
            provider_account_id: "provider-nvidia".to_string(),
            model_code: "nvidia/llama-3.3-nemotron-super-49b-v1.5".to_string(),
            upstream_model: Some("nvidia/llama-3.3-nemotron-super-49b-v1.5".to_string()),
        },
        ProviderCapabilityModelRow {
            provider_account_id: "provider-nvidia".to_string(),
            model_code: "nvidia-llama-3-3-nemotron-super-49b-v1-5".to_string(),
            upstream_model: Some("nvidia/llama-3.3-nemotron-super-49b-v1.5".to_string()),
        },
    ];
    let provider_rows = vec![make_provider_row("provider-nvidia", "openai")];

    let model_ids = collect_model_catalog_ids_for_tests(
        None,
        alias_rows,
        provider_rows,
        capability_rows,
        HashMap::new(),
    );

    assert_eq!(
        model_ids,
        vec!["nvidia-llama-3-3-nemotron-super-49b-v1-5".to_string()]
    );
}

#[test]
fn model_catalog_falls_back_to_provider_default_when_no_platform_model_code_exists() {
    let provider_rows = vec![make_provider_row("provider-openai", "openai")];
    let default_models =
        HashMap::from([("provider-openai".to_string(), Some("gpt-5.4".to_string()))]);

    let model_ids = collect_model_catalog_ids_for_tests(
        None,
        Vec::new(),
        provider_rows,
        Vec::new(),
        default_models,
    );

    assert_eq!(model_ids, vec!["gpt-5.4".to_string()]);
}
