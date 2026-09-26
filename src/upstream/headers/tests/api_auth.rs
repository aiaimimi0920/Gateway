use super::*;

#[test]
fn openai_bearer_auth_by_default() {
    let payload = make_payload("openai_compatible");
    let headers = build_upstream_headers(&payload);
    assert_eq!(
        header_str(&headers, "authorization"),
        Some("Bearer sk-test-1234")
    );
    assert_eq!(
        header_str(&headers, "content-type"),
        Some("application/json")
    );
}

#[test]
fn openai_x_api_key_mode() {
    let mut payload = make_payload("openai_compatible");
    payload.auth_mode = Some("x-api-key".to_string());
    let headers = build_upstream_headers(&payload);
    assert_eq!(header_str(&headers, "x-api-key"), Some("sk-test-1234"));
    // Should NOT set Authorization in x-api-key mode.
    assert!(headers.get("authorization").is_none());
}

#[test]
fn openai_api_key_mode() {
    let mut payload = make_payload("openai_compatible");
    payload.auth_mode = Some("api-key".to_string());
    let headers = build_upstream_headers(&payload);
    assert_eq!(header_str(&headers, "api-key"), Some("sk-test-1234"));
    assert!(headers.get("authorization").is_none());
}

#[test]
fn anthropic_sets_required_headers() {
    let payload = make_payload("anthropic_compatible");
    let headers = build_upstream_headers(&payload);
    assert_eq!(header_str(&headers, "x-api-key"), Some("sk-test-1234"));
    assert_eq!(
        header_str(&headers, "anthropic-version"),
        Some("2023-06-01")
    );
    assert_eq!(
        header_str(&headers, "content-type"),
        Some("application/json")
    );
}

#[test]
fn anthropic_uses_custom_version() {
    let mut payload = make_payload("anthropic_compatible");
    payload.anthropic_version = Some("2024-01-01".to_string());
    let headers = build_upstream_headers(&payload);
    assert_eq!(
        header_str(&headers, "anthropic-version"),
        Some("2024-01-01")
    );
}

#[test]
fn anthropic_joins_beta_headers() {
    let mut payload = make_payload("anthropic_compatible");
    payload.beta_headers = Some(vec![
        "tools-2024-04-04".to_string(),
        "computer-use-2024-10-22".to_string(),
    ]);
    let headers = build_upstream_headers(&payload);
    assert_eq!(
        header_str(&headers, "anthropic-beta"),
        Some("tools-2024-04-04,computer-use-2024-10-22")
    );
}

#[test]
fn anthropic_no_beta_header_when_empty() {
    let mut payload = make_payload("anthropic_compatible");
    payload.beta_headers = Some(vec![]);
    let headers = build_upstream_headers(&payload);
    assert!(headers.get("anthropic-beta").is_none());
}

#[test]
fn gemini_api_defaults_to_x_goog_api_key() {
    let payload = make_payload("gemini_api_compatible");
    let headers = build_upstream_headers(&payload);
    assert_eq!(header_str(&headers, "x-goog-api-key"), Some("sk-test-1234"));
}

#[test]
fn gemini_api_modular_defaults_to_x_goog_api_key() {
    let payload = make_payload("gemini_api_modular_compatible");
    let headers = build_upstream_headers(&payload);
    assert_eq!(header_str(&headers, "x-goog-api-key"), Some("sk-test-1234"));
    assert!(headers.get("authorization").is_none());
}

#[test]
fn gemini_api_can_use_custom_auth_header() {
    let mut payload = make_payload("gemini_api_compatible");
    payload.auth_header_name = Some("authorization".to_string());
    payload.auth_token = Some("Bearer gemini-token".to_string());
    let headers = build_upstream_headers(&payload);
    assert_eq!(
        header_str(&headers, "authorization"),
        Some("Bearer gemini-token")
    );
}

#[test]
fn cohere_defaults_to_bearer_auth() {
    let payload = make_payload("cohere_compatible");
    let headers = build_upstream_headers(&payload);
    assert_eq!(
        header_str(&headers, "authorization"),
        Some("Bearer sk-test-1234")
    );
}

#[test]
fn bedrock_converse_can_use_custom_auth_header() {
    let mut payload = make_payload("bedrock_converse_compatible");
    payload.auth_header_name = Some("x-amz-custom-auth".to_string());
    payload.auth_token = Some("signed-bedrock-token".to_string());
    let headers = build_upstream_headers(&payload);
    assert_eq!(
        header_str(&headers, "x-amz-custom-auth"),
        Some("signed-bedrock-token")
    );
}

#[test]
fn custom_http_uses_auth_header_name_and_token() {
    let mut payload = make_payload("custom_http");
    payload.auth_header_name = Some("x-custom-auth".to_string());
    payload.auth_token = Some("my-token-999".to_string());
    let headers = build_upstream_headers(&payload);
    assert_eq!(header_str(&headers, "x-custom-auth"), Some("my-token-999"));
}

#[test]
fn custom_http_falls_back_to_api_key_when_auth_token_absent() {
    let mut payload = make_payload("custom_http");
    payload.auth_header_name = Some("x-custom-auth".to_string());
    // No auth_token set — falls back to api_key.
    let headers = build_upstream_headers(&payload);
    assert_eq!(header_str(&headers, "x-custom-auth"), Some("sk-test-1234"));
}

#[test]
fn search_api_uses_bearer_auth() {
    let payload = make_payload("search_api_compatible");
    let headers = build_upstream_headers(&payload);
    assert_eq!(
        header_str(&headers, "authorization"),
        Some("Bearer sk-test-1234")
    );
    assert_eq!(
        header_str(&headers, "content-type"),
        Some("application/json")
    );
}

#[test]
fn search_api_compatible_can_use_custom_auth_header() {
    let mut payload = make_payload("search_api_compatible");
    payload.auth_header_name = Some("X-API-Key".to_string());
    let headers = build_upstream_headers(&payload);
    assert_eq!(header_str(&headers, "x-api-key"), Some("sk-test-1234"));
    assert!(headers.get("authorization").is_none());
}

#[test]
fn freebuff_uses_bearer_auth_and_sdk_headers() {
    let mut payload = make_payload("freebuff_compatible");
    payload.extra_body = Some(HashMap::from([(
        "freebuffUserAgent".to_string(),
        serde_json::json!("freebuff-test-agent/1.0"),
    )]));

    let headers = build_upstream_headers(&payload);

    assert_eq!(
        header_str(&headers, "authorization"),
        Some("Bearer sk-test-1234")
    );
    assert_eq!(
        header_str(&headers, "accept"),
        Some("application/json, text/event-stream")
    );
    assert_eq!(
        header_str(&headers, "user-agent"),
        Some("freebuff-test-agent/1.0")
    );
}

#[test]
fn kiro_uses_runtime_bearer_and_aws_headers() {
    let mut payload = make_payload("kiro_compatible");
    payload.session_auth = Some(crate::credential_runtime::SessionAuthConfig {
        transport: "bearer".to_string(),
        primary_cookie_name: None,
        secondary_cookie_name: None,
        header_name: Some("authorization".to_string()),
        expires_at: None,
    });
    payload.extra_body = Some(HashMap::from([
        (
            "kiroRefreshToken".to_string(),
            serde_json::json!("refresh-123"),
        ),
        (
            "kiroApiRegion".to_string(),
            serde_json::json!("eu-central-1"),
        ),
    ]));

    let headers = build_upstream_headers(&payload);

    assert_eq!(
        header_str(&headers, "authorization"),
        Some("Bearer sk-test-1234")
    );
    assert_eq!(
        header_str(&headers, "x-amzn-codewhisperer-optout"),
        Some("true")
    );
    assert_eq!(header_str(&headers, "x-amzn-kiro-agent-mode"), Some("vibe"));
    assert_eq!(
        header_str(&headers, "host"),
        Some("q.eu-central-1.amazonaws.com")
    );
    assert!(header_str(&headers, "x-amz-user-agent").is_some());
    assert!(header_str(&headers, "user-agent").is_some());
    assert!(header_str(&headers, "amz-sdk-invocation-id").is_some());
}
