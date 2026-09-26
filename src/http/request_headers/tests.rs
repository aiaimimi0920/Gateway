use super::*;
use crate::protocol::canonical::{
    CanonicalMessage, CanonicalRelayRequest, ContentPart, EndpointKind, MessageRole, ProtocolFamily,
};
use std::collections::HashMap;

fn make_config(aliases: &[&str]) -> Config {
    make_config_with_management_token(aliases, None)
}

fn make_config_with_management_token(
    aliases: &[&str],
    gateway_management_token: Option<&str>,
) -> Config {
    Config {
        console: Default::default(),
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
        gateway_management_token: gateway_management_token.map(str::to_string),
        gateway_keepalive_bearer_token: None,
        default_project_id: "platform-default-project".to_string(),
        gateway_inbound_api_key_header_aliases: aliases
            .iter()
            .map(|value| value.to_ascii_lowercase())
            .collect(),
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

fn make_ctx() -> PipelineContext {
    PipelineContext::new(
        CanonicalRelayRequest {
            protocol_family: ProtocolFamily::OpenAi,
            endpoint_kind: EndpointKind::ChatCompletions,
            requested_model: Some("gpt-4o".to_string()),
            stream: false,
            messages: vec![CanonicalMessage {
                role: MessageRole::User,
                content: vec![ContentPart::Text {
                    text: "hello".to_string(),
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
        },
        None,
    )
}

fn apply_with_config(
    ctx: &mut PipelineContext,
    headers: &HeaderMap,
    config: &Config,
    extra_headers: &[&str],
) -> Result<(), GatewayError> {
    let fixture = match config.gateway_management_token.as_deref() {
        Some(token) => TestConsoleAuthFixture::with_environment_token(token),
        None => TestConsoleAuthFixture::without_management_token(),
    };
    apply_public_request_headers(ctx, headers, config, &fixture.runtime, extra_headers)
}

#[test]
fn extract_inbound_api_key_supports_api_key_header() {
    let mut headers = HeaderMap::new();
    headers.insert("api-key", "azure-style-key".parse().unwrap());
    let config = make_config(&[]);

    assert_eq!(
        extract_inbound_api_key(&headers, &config).as_deref(),
        Some("azure-style-key")
    );
}

#[test]
fn extract_inbound_api_key_supports_x_goog_api_key_header() {
    let mut headers = HeaderMap::new();
    headers.insert("X-Goog-Api-Key", "google-style-key".parse().unwrap());
    let config = make_config(&[]);

    assert_eq!(
        extract_inbound_api_key(&headers, &config).as_deref(),
        Some("google-style-key")
    );
}

#[test]
fn extract_inbound_api_key_supports_custom_aliases() {
    let mut headers = HeaderMap::new();
    headers.insert("x-auth-token", "alias-key".parse().unwrap());
    let config = make_config(&["x-auth-token"]);

    assert_eq!(
        extract_inbound_api_key(&headers, &config).as_deref(),
        Some("alias-key")
    );
}

#[test]
fn apply_public_request_headers_canonicalizes_api_key_without_trusting_forwarded_headers() {
    let mut ctx = make_ctx();
    let mut headers = HeaderMap::new();
    headers.insert("api-key", "azure-style-key".parse().unwrap());
    headers.insert("x-credential-ref", "cred-1".parse().unwrap());
    headers.insert("x-neuro-user", "forged-user".parse().unwrap());
    headers.insert("x-neuro-cred-source", "hosted".parse().unwrap());
    let config = make_config(&[]);

    apply_with_config(
        &mut ctx,
        &headers,
        &config,
        &["x-neuro-account-group", "x-account-group-id"],
    )
    .unwrap();

    assert_eq!(
        ctx.request_headers.get("x-api-key").map(String::as_str),
        Some("azure-style-key")
    );
    assert_eq!(
        ctx.request_headers
            .get("x-credential-ref")
            .map(String::as_str),
        None
    );
    assert_eq!(ctx.request_headers.get("x-neuro-user"), None);
    assert_eq!(ctx.request_headers.get("x-neuro-cred-source"), None);
    assert_eq!(ctx.request_headers.get("x-neuro-account-group"), None);
    assert_eq!(ctx.account_group_id, None);
}

#[test]
fn apply_public_request_headers_keeps_account_group_headers_out_of_generic_forwarding() {
    let mut ctx = make_ctx();
    let mut headers = HeaderMap::new();
    headers.insert("x-api-key", "public-key".parse().unwrap());
    headers.insert("x-internal-api-key", "management-token".parse().unwrap());
    headers.insert("x-credential-ref", "cred-1".parse().unwrap());
    headers.insert("x-neuro-user", "trusted-user".parse().unwrap());
    headers.insert("x-neuro-cred-source", "hosted".parse().unwrap());
    headers.insert("x-account-group-id", "premium".parse().unwrap());
    let config = make_config_with_management_token(&[], Some("management-token"));

    apply_with_config(&mut ctx, &headers, &config, &[]).unwrap();

    assert_eq!(
        ctx.request_headers.get("x-api-key").map(String::as_str),
        Some("public-key")
    );
    assert_eq!(
        ctx.request_headers
            .get("x-credential-ref")
            .map(String::as_str),
        Some("cred-1")
    );
    assert_eq!(
        ctx.request_headers.get("x-neuro-user").map(String::as_str),
        Some("trusted-user")
    );
    assert_eq!(
        ctx.request_headers
            .get("x-neuro-cred-source")
            .map(String::as_str),
        Some("hosted")
    );
    assert_eq!(ctx.request_headers.get("x-neuro-account-group"), None);
    assert_eq!(ctx.request_headers.get("x-account-group-id"), None);
    assert_eq!(ctx.account_group_id.as_deref(), Some("premium"));
}

#[test]
fn apply_public_request_headers_ignores_forwarded_headers_with_wrong_management_token() {
    let mut ctx = make_ctx();
    let mut headers = HeaderMap::new();
    headers.insert("x-api-key", "public-key".parse().unwrap());
    headers.insert("x-internal-api-key", "wrong-token".parse().unwrap());
    headers.insert("x-credential-ref", "cred-1".parse().unwrap());
    headers.insert("x-neuro-user", "forged-user".parse().unwrap());
    headers.insert("x-neuro-cred-source", "hosted".parse().unwrap());
    let config = make_config_with_management_token(&[], Some("management-token"));

    apply_with_config(&mut ctx, &headers, &config, &[]).unwrap();

    assert_eq!(
        ctx.request_headers.get("x-api-key").map(String::as_str),
        Some("public-key")
    );
    assert_eq!(ctx.request_headers.get("x-credential-ref"), None);
    assert_eq!(ctx.request_headers.get("x-neuro-user"), None);
    assert_eq!(ctx.request_headers.get("x-neuro-cred-source"), None);
    assert_eq!(ctx.request_headers.get("x-neuro-account-group"), None);
    assert_eq!(ctx.account_group_id, None);
}

#[test]
fn apply_public_request_headers_canonicalizes_x_goog_api_key() {
    let mut ctx = make_ctx();
    let mut headers = HeaderMap::new();
    headers.insert("X-Goog-Api-Key", "google-style-key".parse().unwrap());
    let config = make_config(&[]);

    apply_with_config(&mut ctx, &headers, &config, &[]).unwrap();

    assert_eq!(
        ctx.request_headers.get("x-api-key").map(String::as_str),
        Some("google-style-key")
    );
}

#[test]
fn apply_public_request_headers_forwards_request_and_trace_context() {
    let mut ctx = make_ctx();
    let mut headers = HeaderMap::new();
    headers.insert("x-request-id", "request-42".parse().unwrap());
    headers.insert(
        "traceparent",
        "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"
            .parse()
            .unwrap(),
    );
    headers.insert("tracestate", "vendor=value".parse().unwrap());
    let config = make_config(&[]);

    apply_with_config(&mut ctx, &headers, &config, &[]).unwrap();

    assert_eq!(
        ctx.request_headers.get("x-request-id").map(String::as_str),
        Some("request-42")
    );
    assert_eq!(
        ctx.request_headers.get("traceparent").map(String::as_str),
        Some("00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01")
    );
    assert_eq!(
        ctx.request_headers.get("tracestate").map(String::as_str),
        Some("vendor=value")
    );
}

#[test]
fn account_group_selector_accepts_environment_management_token() {
    let fixture = TestConsoleAuthFixture::with_environment_token("environment-token");
    let mut ctx = make_ctx();
    let mut headers = HeaderMap::new();
    headers.insert("x-neuro-account-group", "premium".parse().unwrap());
    headers.insert("x-internal-api-key", "environment-token".parse().unwrap());

    apply_public_request_headers(&mut ctx, &headers, &make_config(&[]), &fixture.runtime, &[])
        .expect("environment token selector");

    assert_eq!(ctx.account_group_id.as_deref(), Some("premium"));
    assert_eq!(ctx.request_headers.get("x-neuro-account-group"), None);
}

#[test]
fn account_group_selector_accepts_bootstrapped_management_token() {
    let fixture = TestConsoleAuthFixture::with_bootstrapped_token("bootstrap-token");
    let mut ctx = make_ctx();
    let mut headers = HeaderMap::new();
    headers.insert("x-account-group-id", "isolated".parse().unwrap());
    headers.insert("x-internal-api-key", "bootstrap-token".parse().unwrap());

    apply_public_request_headers(&mut ctx, &headers, &make_config(&[]), &fixture.runtime, &[])
        .expect("bootstrap token selector");

    assert_eq!(ctx.account_group_id.as_deref(), Some("isolated"));
    assert_eq!(ctx.request_headers.get("x-account-group-id"), None);
}

#[test]
fn explicit_account_group_selector_without_token_fails_closed() {
    let fixture = TestConsoleAuthFixture::without_management_token();
    let mut ctx = make_ctx();
    let mut headers = HeaderMap::new();
    headers.insert("x-neuro-account-group", "premium".parse().unwrap());

    let error =
        apply_public_request_headers(&mut ctx, &headers, &make_config(&[]), &fixture.runtime, &[])
            .expect_err("explicit selector without management token must fail closed");

    assert_eq!(error.http_status, Some(401));
    assert_eq!(
        error.code.as_deref(),
        Some("console_management_token_invalid")
    );
    assert_eq!(ctx.account_group_id, None);
}

#[test]
fn explicit_account_group_selector_with_invalid_token_fails_closed() {
    let fixture = TestConsoleAuthFixture::with_environment_token("environment-token");
    let mut ctx = make_ctx();
    let mut headers = HeaderMap::new();
    headers.insert("x-neuro-account-group", "premium".parse().unwrap());
    headers.insert("x-internal-api-key", "wrong-token".parse().unwrap());

    let error =
        apply_public_request_headers(&mut ctx, &headers, &make_config(&[]), &fixture.runtime, &[])
            .expect_err("explicit selector with invalid management token must fail closed");

    assert_eq!(error.http_status, Some(401));
    assert_eq!(
        error.code.as_deref(),
        Some("console_management_token_invalid")
    );
    assert_eq!(ctx.account_group_id, None);
}

#[test]
fn request_without_account_group_selector_preserves_compatibility() {
    let fixture = TestConsoleAuthFixture::without_management_token();
    let mut ctx = make_ctx();
    let mut headers = HeaderMap::new();
    headers.insert("x-internal-api-key", "wrong-token".parse().unwrap());
    headers.insert("x-request-id", "request-42".parse().unwrap());

    apply_public_request_headers(&mut ctx, &headers, &make_config(&[]), &fixture.runtime, &[])
        .expect("requests without selectors must preserve existing compatibility");

    assert_eq!(ctx.account_group_id, None);
    assert_eq!(
        ctx.request_headers.get("x-request-id").map(String::as_str),
        Some("request-42")
    );
}
