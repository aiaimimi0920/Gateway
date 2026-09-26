use super::*;

#[test]
fn compile_accio_account_merges_token_into_extra_body() {
    let preset = accio_preset();
    let mut extra_body = HashMap::new();
    extra_body.insert("token".to_string(), Value::String("abc123".to_string()));

    let mut account_headers = HashMap::new();
    account_headers.insert("utdid".to_string(), "utd-xxx".to_string());

    let account = AccountOverrides {
        base_url: "https://phoenix-gw.alibaba.com".to_string(),
        api_key: "unused".to_string(),
        headers: account_headers,
        extra_body,
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
    assert_eq!(compiled.adapter, "accio_compatible");
    assert_eq!(compiled.base_url, "https://phoenix-gw.alibaba.com");
    assert_eq!(compiled.headers.get("utdid").unwrap(), "utd-xxx");
    assert_eq!(compiled.headers.get("version").unwrap(), "0.5.6");
    assert!(compiled.headers.get("appKey").is_none());

    let extra = compiled.extra_body.as_ref().unwrap();
    assert_eq!(
        extra.get("token").unwrap(),
        &Value::String("abc123".to_string())
    );
}

#[test]
fn compile_qwen_web_account_uses_qwen_web_adapter() {
    let preset = qwen_web_chat_preset();
    let account = AccountOverrides {
        base_url: "https://chat.qwen.ai".to_string(),
        api_key: "access-token-xxx".to_string(),
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
    assert_eq!(compiled.adapter, "qwen_web_compatible");
    assert_eq!(compiled.api_key, "access-token-xxx");
    assert_eq!(
        compiled.headers.get("source").map(String::as_str),
        Some("web")
    );
    assert_eq!(
        compiled
            .session_auth
            .as_ref()
            .and_then(SessionAuthConfig::header_name),
        Some("authorization")
    );
}

#[test]
fn compile_grok_account_keeps_sso_token_in_api_key() {
    let preset = grok_preset();
    let account = AccountOverrides {
        base_url: "https://grok.com".to_string(),
        api_key: "sso-token-xxx".to_string(),
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
    assert_eq!(compiled.adapter, "grok_compatible");
    assert_eq!(compiled.base_url, "https://grok.com");
    assert_eq!(compiled.api_key, "sso-token-xxx");
    assert_eq!(
        compiled
            .session_auth
            .as_ref()
            .map(SessionAuthConfig::primary_cookie_name),
        Some("sso")
    );
    assert_eq!(
        compiled.chat_completions_path.as_deref(),
        Some("/rest/app-chat/conversations/new")
    );
    assert_eq!(
        compiled.headers.get("Accept").map(String::as_str),
        Some("text/event-stream")
    );
}

#[test]
fn compile_gemini_canvas_account_preserves_browser_runtime_metadata() {
    let preset = gemini_canvas_preset();
    let account = AccountOverrides {
        base_url: "https://gemini.google.com".to_string(),
        api_key: String::new(),
        headers: HashMap::new(),
        extra_body: HashMap::new(),
        default_model: None,
        auth_mode: None,
        anthropic_version: None,
        beta_headers: None,
        auth_header_name: None,
        auth_token: None,
        session_auth: None,
        keepalive: Some(KeepaliveConfig {
            service_url: "http://gateway.internal".to_string(),
            ensure_path: None,
            auth_token: None,
            timeout_secs: None,
            refresh_before_secs: Some(300),
        }),
        expires_at: Some("2099-01-01T00:00:00.000Z".to_string()),
        runtime_state_object_key: Some("objects/gemini-canvas/auth-1.json".to_string()),
        account_name: Some("canvas-main".to_string()),
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
    assert_eq!(compiled.adapter, "gemini_canvas_compatible");
    assert_eq!(
        compiled.runtime_state_object_key.as_deref(),
        Some("objects/gemini-canvas/auth-1.json")
    );
    assert_eq!(compiled.account_name.as_deref(), Some("canvas-main"));
    assert_eq!(
        compiled.expires_at.as_deref(),
        Some("2099-01-01T00:00:00.000Z")
    );
}
