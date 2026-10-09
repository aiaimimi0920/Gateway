use super::super::*;

fn qwen_payload(expires_at: Option<&str>) -> ProviderAccountPayload {
    ProviderAccountPayload {
        discovered_protocols: Vec::new(),
        adapter: "qwen_web_compatible".to_string(),
        base_url: "https://chat.qwen.ai".to_string(),
        api_key: "session-token".to_string(),
        credential_id: Some("cred-1".to_string()),
        expires_at: expires_at.map(str::to_string),
        runtime_state_object_key: None,
        account_name: Some("qwen-web".to_string()),
        execution_mode: None,
        endpoint_execution_modes: None,
        default_model: Some("qwen3-coder-plus".to_string()),
        headers: HashMap::new(),
        auth_mode: Some("bearer".to_string()),
        anthropic_version: None,
        beta_headers: None,
        auth_header_name: None,
        auth_token: None,
        responses_path: None,
        chat_completions_path: Some("/api/v2/chat/completions".to_string()),
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
        extra_body: None,
        session_auth: Some(SessionAuthConfig {
            transport: "bearer".to_string(),
            primary_cookie_name: None,
            secondary_cookie_name: None,
            header_name: Some("authorization".to_string()),
            expires_at: expires_at.map(str::to_string),
        }),
        keepalive: None,
    }
}

#[test]
fn qwen_web_refresh_forces_when_expired() {
    let payload = qwen_payload(Some("2000-01-01T00:00:00.000Z"));
    assert!(qwen_web_should_refresh(&payload, false));
}

#[test]
fn qwen_web_refresh_skips_when_session_is_fresh() {
    let payload = qwen_payload(Some("2099-01-01T00:00:00.000Z"));
    assert!(!qwen_web_should_refresh(&payload, false));
}

#[test]
fn qwen_web_refresh_skips_when_valid_token_has_no_expiry_metadata() {
    let payload = qwen_payload(None);
    assert!(!qwen_web_should_refresh(&payload, false));
}

#[test]
fn qwen_web_refresh_merges_cookie_header() {
    let mut headers = HashMap::new();
    headers.insert("Accept".to_string(), "application/json".to_string());
    headers.insert("x-neuro-account-group".to_string(), "premium".to_string());
    let merged = merge_qwen_web_runtime_headers(&headers, Some("token=abc"));
    assert_eq!(
        merged.get("Accept").map(String::as_str),
        Some("application/json")
    );
    assert_eq!(merged.get("Cookie").map(String::as_str), Some("token=abc"));
    assert!(!merged
        .keys()
        .any(|name| name.eq_ignore_ascii_case("x-neuro-account-group")));
}

#[test]
fn qwen_web_signin_headers_drop_internal_account_group_selectors() {
    let mut payload = qwen_payload(None);
    payload
        .headers
        .insert("x-neuro-account-group".to_string(), "premium".to_string());
    payload.headers.insert(
        "X-Account-Group-Id".to_string(),
        "legacy-premium".to_string(),
    );
    payload
        .headers
        .insert("x-provider-runtime".to_string(), "allowed".to_string());

    let result = qwen_web_signin_headers(&payload);

    assert!(!result
        .keys()
        .any(|name| name.eq_ignore_ascii_case("x-neuro-account-group")));
    assert!(!result
        .keys()
        .any(|name| name.eq_ignore_ascii_case("x-account-group-id")));
    assert_eq!(
        result.get("x-provider-runtime").map(String::as_str),
        Some("allowed")
    );
}

#[test]
fn qwen_web_signin_seed_reads_nested_auth_seed() {
    let mut extra_body = HashMap::new();
    extra_body.insert(
        "authSeed".to_string(),
        serde_json::json!({
            "type": "qwen_web_signin",
            "email": "user@example.com",
            "password": "secret-123"
        }),
    );
    let seed = qwen_web_signin_seed(Some(&extra_body)).expect("seed");
    assert_eq!(seed.email, "user@example.com");
    assert_eq!(seed.password.as_deref(), Some("secret-123"));
    assert_eq!(seed.password_sha256.as_deref(), None);
}

#[test]
fn qwen_web_signin_password_attempts_include_plain_and_hash() {
    let seed = QwenWebSigninSeed {
        email: "user@example.com".to_string(),
        password: Some("secret-123".to_string()),
        password_sha256: None,
    };
    let attempts = qwen_web_signin_password_attempts(&seed);
    assert_eq!(attempts.len(), 2);
    assert_eq!(attempts[0], "secret-123");
    assert_eq!(attempts[1], sha256_hex("secret-123"));
}
