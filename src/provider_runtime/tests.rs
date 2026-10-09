use super::recording::is_permanent_provider_credential_failure;
use super::*;
use crate::routing::candidate::ProviderAccountPayload;
use crate::routing::candidate::ProviderExecutionMode;
use std::collections::HashMap;
use std::time::Duration;

fn codex_payload(base_url: &str, originator: Option<&str>) -> ProviderAccountPayload {
    let mut headers = HashMap::new();
    if let Some(originator) = originator {
        headers.insert("Originator".to_string(), originator.to_string());
    }
    ProviderAccountPayload {
        discovered_protocols: Vec::new(),
        adapter: "openai_compatible".to_string(),
        base_url: base_url.to_string(),
        api_key: "tok".to_string(),
        credential_id: None,
        default_model: Some("gpt-5.4".to_string()),
        headers,
        extra_body: None,
        auth_mode: None,
        anthropic_version: None,
        beta_headers: None,
        auth_header_name: None,
        auth_token: None,
        responses_path: Some("/responses".to_string()),
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
        session_auth: None,
        keepalive: None,
        execution_mode: Some(ProviderExecutionMode::DirectHttp),
        endpoint_execution_modes: None,
        expires_at: None,
        runtime_state_object_key: None,
        account_name: None,
    }
}

#[test]
fn recognizes_chatgpt_codex_payload() {
    let payload = codex_payload(
        "https://chatgpt.com/backend-api/codex",
        Some("codex_cli_rs"),
    );
    assert!(crate::protocol::chatgpt::official_api::is_chatgpt_codex_backend_payload(&payload));
}

#[test]
fn does_not_classify_openai_as_codex_payload() {
    let payload = codex_payload("https://api.openai.com/v1", Some("codex_cli_rs"));
    assert!(!crate::protocol::chatgpt::official_api::is_chatgpt_codex_backend_payload(&payload));
}

#[test]
fn classifies_deactivated_workspace_as_permanent_credential_failure() {
    assert!(is_permanent_provider_credential_failure(
        r#"{"detail":{"code":"deactivated_workspace"}}"#
    ));
}

#[test]
fn classifies_token_invalidated_as_permanent_credential_failure() {
    assert!(is_permanent_provider_credential_failure(
        "Your authentication token has been invalidated. Please try signing in again."
    ));
}

#[test]
fn classifies_appid_no_auth_error_as_permanent_credential_failure() {
    assert!(is_permanent_provider_credential_failure(
        "AppIdNoAuthError: xop35qwen2b is not authorized for this appid"
    ));
}

#[test]
fn persistence_boundary_redacts_provider_error_secrets() {
    let sanitized = provider_error_message_for_persistence(
        "invalid api key; Authorization: Bearer persisted-secret; session=private-session",
    );

    assert!(sanitized.contains("invalid api key"));
    assert!(sanitized.contains("[REDACTED]"));
    assert!(!sanitized.contains("persisted-secret"));
    assert!(!sanitized.contains("private-session"));
}

fn probe_test_client() -> rquest::Client {
    rquest::Client::builder()
        .timeout(Duration::from_millis(100))
        .build()
        .expect("probe test client")
}

#[tokio::test]
async fn console_probe_rejects_any_browser_backed_execution_mode_without_network() {
    let mut execution_mode_payload = codex_payload("http://127.0.0.1:1", None);
    execution_mode_payload.execution_mode = Some(ProviderExecutionMode::BrowserBacked);

    let mut endpoint_mode_payload = codex_payload("http://127.0.0.1:1", None);
    endpoint_mode_payload.endpoint_execution_modes = Some(HashMap::from([(
        "chat_completions".to_string(),
        ProviderExecutionMode::BrowserBacked,
    )]));

    for payload in [execution_mode_payload, endpoint_mode_payload] {
        let report = probe_provider_payload_for_console(&probe_test_client(), &payload).await;
        assert_eq!(report.status, ProviderPayloadProbeStatus::Unsupported);
        assert!(report.message.contains("browser-backed"));
    }
}

#[tokio::test]
async fn strict_console_probe_does_not_probe_grok_or_generic_http_adapters() {
    for adapter in ["grok_compatible", "custom_http", "provider_passthrough"] {
        let mut payload = codex_payload("http://127.0.0.1:1", None);
        payload.adapter = adapter.to_string();

        let report = probe_provider_payload_for_console(&probe_test_client(), &payload).await;
        assert_eq!(report.status, ProviderPayloadProbeStatus::Unsupported);
        assert!(report.message.contains(adapter));
    }
}

#[tokio::test]
async fn strict_search_probe_requires_explicit_balance_path() {
    let mut payload = codex_payload("http://127.0.0.1:1", None);
    payload.adapter = "search_api_compatible".to_string();
    payload.search_path = Some("/search".to_string());
    payload.balance_path = None;

    let report = probe_provider_payload_for_console(&probe_test_client(), &payload).await;
    assert_eq!(report.status, ProviderPayloadProbeStatus::Unsupported);
    assert!(report.message.contains("balance_path"));
}

#[tokio::test]
async fn strict_console_probe_does_not_run_freebuff_stateful_probe() {
    let mut payload = codex_payload("http://127.0.0.1:1", None);
    payload.adapter = "freebuff_compatible".to_string();

    let report = probe_provider_payload_for_console(&probe_test_client(), &payload).await;

    assert_eq!(report.status, ProviderPayloadProbeStatus::Unsupported);
    assert!(report.message.contains("stateful"));
    assert!(report.message.contains("freebuff_compatible"));
}

#[test]
fn console_probe_point_matches_the_strict_http_endpoint() {
    let openai = codex_payload("https://gateway.example/v1/", None);
    assert_eq!(
        provider_payload_probe_point(&openai),
        "GET https://gateway.example/v1/models"
    );

    let mut search = codex_payload("https://search.example/api", None);
    search.adapter = "search_api_compatible".to_string();
    search.balance_path = Some("/credits".to_string());
    assert_eq!(
        provider_payload_probe_point(&search),
        "GET https://search.example/api/credits"
    );

    search.balance_path = Some("/credits?token=probe-secret&view=summary".to_string());
    let point = provider_payload_probe_point(&search);
    assert!(point.contains("view=summary"));
    assert!(!point.contains("probe-secret"));
    assert!(!point.contains("token="));
}
