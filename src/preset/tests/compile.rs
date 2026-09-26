use super::*;

#[test]
fn compile_codex_account_has_correct_headers_and_extra_body() {
    let preset = codex_preset();
    let mut account_headers = HashMap::new();
    account_headers.insert("Chatgpt-Account-Id".to_string(), "abc-123".to_string());

    let account = AccountOverrides {
        base_url: "https://chatgpt.com/backend-api/codex".to_string(),
        api_key: "tok_xxx".to_string(),
        headers: account_headers,
        extra_body: HashMap::new(),
        default_model: None,
        auth_mode: None,
        anthropic_version: None,
        beta_headers: None,
        auth_header_name: None,
        auth_token: None,
        session_auth: None,
        keepalive: None,
        expires_at: None,
        runtime_state_object_key: None,
        account_name: None,
        search_path: None,
        fetch_path: None,
        research_path: None,
        balance_path: None,
        search_query_field: None,
        fetch_urls_field: None,
        execution_mode: None,
        endpoint_execution_modes: None,
    };

    let compiled = compile_provider_account(&preset, &account);
    assert_eq!(compiled.adapter, "openai_compatible");
    assert_eq!(compiled.base_url, "https://chatgpt.com/backend-api/codex");
    assert_eq!(compiled.api_key, "tok_xxx");
    assert_eq!(
        compiled.headers.get("User-Agent").unwrap(),
        "codex_cli_rs/0.116.0 (Mac OS 26.0.1; arm64) Apple_Terminal/464"
    );
    assert_eq!(compiled.headers.get("Originator").unwrap(), "codex_cli_rs");
    assert_eq!(
        compiled.headers.get("Chatgpt-Account-Id").unwrap(),
        "abc-123"
    );
    let extra = compiled.extra_body.as_ref().unwrap();
    assert_eq!(extra.get("store").unwrap(), &Value::Bool(false));
}

#[test]
fn compile_second_codex_site_only_differs_in_base_url() {
    let preset = codex_preset();

    let site_a = AccountOverrides {
        base_url: "https://chatgpt.com/backend-api/codex".to_string(),
        api_key: "tok_a".to_string(),
        headers: HashMap::new(),
        extra_body: HashMap::new(),
        default_model: None,
        auth_mode: None,
        anthropic_version: None,
        beta_headers: None,
        auth_header_name: None,
        auth_token: None,
        session_auth: None,
        keepalive: None,
        expires_at: None,
        runtime_state_object_key: None,
        account_name: None,
        search_path: None,
        fetch_path: None,
        research_path: None,
        balance_path: None,
        search_query_field: None,
        fetch_urls_field: None,
        execution_mode: None,
        endpoint_execution_modes: None,
    };
    let site_b = AccountOverrides {
        base_url: "https://mirror.example.com/codex".to_string(),
        api_key: "tok_b".to_string(),
        ..site_a.clone()
    };

    let a = compile_provider_account(&preset, &site_a);
    let b = compile_provider_account(&preset, &site_b);

    // Same preset-derived config
    assert_eq!(a.adapter, b.adapter);
    assert_eq!(a.headers.get("User-Agent"), b.headers.get("User-Agent"));
    assert_eq!(a.headers.get("Originator"), b.headers.get("Originator"));
    assert_eq!(a.extra_body, b.extra_body);

    // Different account-specific config
    assert_ne!(a.base_url, b.base_url);
    assert_ne!(a.api_key, b.api_key);
}

#[test]
fn account_headers_override_preset_headers() {
    let preset = codex_preset();
    let mut h = HashMap::new();
    h.insert("User-Agent".to_string(), "my-custom-agent/1.0".to_string());

    let account = AccountOverrides {
        base_url: "https://example.com".to_string(),
        api_key: "k".to_string(),
        headers: h,
        extra_body: HashMap::new(),
        default_model: None,
        auth_mode: None,
        anthropic_version: None,
        beta_headers: None,
        auth_header_name: None,
        auth_token: None,
        session_auth: None,
        keepalive: None,
        expires_at: None,
        runtime_state_object_key: None,
        account_name: None,
        search_path: None,
        fetch_path: None,
        research_path: None,
        balance_path: None,
        search_query_field: None,
        fetch_urls_field: None,
        execution_mode: None,
        endpoint_execution_modes: None,
    };

    let compiled = compile_provider_account(&preset, &account);
    assert_eq!(
        compiled.headers.get("User-Agent").unwrap(),
        "my-custom-agent/1.0"
    );
    // Originator still inherited from preset
    assert_eq!(compiled.headers.get("Originator").unwrap(), "codex_cli_rs");
}

#[test]
fn account_default_model_overrides_preset() {
    let preset = codex_preset();
    let account = AccountOverrides {
        base_url: "https://example.com".to_string(),
        api_key: "k".to_string(),
        headers: HashMap::new(),
        extra_body: HashMap::new(),
        default_model: Some("gpt-5.1-codex".to_string()),
        auth_mode: None,
        anthropic_version: None,
        beta_headers: None,
        auth_header_name: None,
        auth_token: None,
        session_auth: None,
        keepalive: None,
        expires_at: None,
        runtime_state_object_key: None,
        account_name: None,
        search_path: None,
        fetch_path: None,
        research_path: None,
        balance_path: None,
        search_query_field: None,
        fetch_urls_field: None,
        execution_mode: None,
        endpoint_execution_modes: None,
    };

    let compiled = compile_provider_account(&preset, &account);
    assert_eq!(compiled.default_model.as_deref(), Some("gpt-5.1-codex"));
}

#[test]
fn preset_default_model_used_when_account_has_none() {
    let preset = codex_preset();
    let account = AccountOverrides {
        base_url: "https://example.com".to_string(),
        api_key: "k".to_string(),
        headers: HashMap::new(),
        extra_body: HashMap::new(),
        default_model: None,
        auth_mode: None,
        anthropic_version: None,
        beta_headers: None,
        auth_header_name: None,
        auth_token: None,
        session_auth: None,
        keepalive: None,
        expires_at: None,
        runtime_state_object_key: None,
        account_name: None,
        search_path: None,
        fetch_path: None,
        research_path: None,
        balance_path: None,
        search_query_field: None,
        fetch_urls_field: None,
        execution_mode: None,
        endpoint_execution_modes: None,
    };

    let compiled = compile_provider_account(&preset, &account);
    assert_eq!(compiled.default_model.as_deref(), Some("gpt-5.4"));
}

#[test]
fn empty_extra_body_compiles_to_none() {
    let preset = openai_preset();
    let account = AccountOverrides {
        base_url: "https://api.openai.com".to_string(),
        api_key: "sk-xxx".to_string(),
        headers: HashMap::new(),
        extra_body: HashMap::new(),
        default_model: None,
        auth_mode: None,
        anthropic_version: None,
        beta_headers: None,
        auth_header_name: None,
        auth_token: None,
        session_auth: None,
        keepalive: None,
        expires_at: None,
        runtime_state_object_key: None,
        account_name: None,
        search_path: None,
        fetch_path: None,
        research_path: None,
        balance_path: None,
        search_query_field: None,
        fetch_urls_field: None,
        execution_mode: None,
        endpoint_execution_modes: None,
    };

    let compiled = compile_provider_account(&preset, &account);
    assert!(compiled.extra_body.is_none());
}

#[test]
fn preset_serialization_roundtrip() {
    let preset = codex_preset();
    let json = serde_json::to_string(&preset).unwrap();
    let restored: ProviderPreset = serde_json::from_str(&json).unwrap();
    assert_eq!(preset.id, restored.id);
    assert_eq!(preset.adapter, restored.adapter);
    assert_eq!(preset.headers, restored.headers);
}
