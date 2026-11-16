use std::collections::HashMap;

use serde_json::{json, Value};

use super::{parse_chataibot_http_response, unsupported_request_plan_error};
use crate::error::ErrorKind;
use crate::protocol::canonical::{CanonicalRelayRequest, EndpointKind, ProtocolFamily};
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::client::UpstreamClient;

fn make_payload(adapter: &str, base_url: &str) -> ProviderAccountPayload {
    ProviderAccountPayload {
        call_flow: None,
        request_client_profile: None,
        discovered_protocols: Vec::new(),
        adapter: adapter.to_string(),
        base_url: base_url.to_string(),
        api_key: "sk-test".to_string(),
        credential_id: None,
        expires_at: None,
        runtime_state_object_key: None,
        account_name: None,
        execution_mode: None,
        endpoint_execution_modes: None,
        default_model: None,
        headers: HashMap::new(),
        auth_mode: None,
        anthropic_version: None,
        beta_headers: None,
        auth_header_name: None,
        auth_token: None,
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
        extra_body: None,
        session_auth: None,
        keepalive: None,
    }
}

fn make_request(protocol: ProtocolFamily, endpoint: EndpointKind) -> CanonicalRelayRequest {
    CanonicalRelayRequest {
        protocol_family: protocol,
        endpoint_kind: endpoint,
        requested_model: Some("test-model".to_string()),
        stream: false,
        messages: vec![],
        tools: vec![],
        tool_choice: None,
        reasoning: None,
        metadata: None,
        raw_body: json!({}),
        previous_response_id: None,
        explicit_session_key: None,
        extra: HashMap::new(),
    }
}

#[test]
fn chataibot_unsupported_request_plan_error_matches_contract() {
    let err = unsupported_request_plan_error();
    assert_eq!(err.http_status, Some(400));
    assert_eq!(err.code.as_deref(), Some("unsupported_chataibot_endpoint"));
    assert!(err.message.contains("image generation/edit"));
}

#[test]
fn plan_chataibot_chat_endpoint_rejected_locally() {
    let payload = make_payload("chataibot_compatible", "https://chataibot.pro");
    let req = make_request(ProtocolFamily::OpenAi, EndpointKind::ChatCompletions);
    let err = UpstreamClient::build_request_plan(&payload, &req, "google-nano-banana", false)
        .expect_err("chataibot chat requests should be rejected");
    assert_eq!(err.http_status, Some(400));
    assert_eq!(err.code.as_deref(), Some("unsupported_chataibot_endpoint"));
}

#[tokio::test]
async fn execute_chataibot_mask_edit_rejected_before_send() {
    let payload = make_payload("chataibot_compatible", "https://chataibot.pro");
    let mut req = make_request(ProtocolFamily::OpenAi, EndpointKind::ImagesEdits);
    req.requested_model = Some("qwen-lora".to_string());
    req.raw_body = json!({
        "prompt": "mask should fail",
        "image": "data:image/png;base64,aGVsbG8=",
        "mask": "data:image/png;base64,aGVsbG8=",
    });
    let client = UpstreamClient::new(5);
    let err = client
        .execute_chataibot_images(&payload, &req, "qwen-lora", None)
        .await
        .expect_err("mask-based chataibot edits should be rejected");
    assert_eq!(err.http_status, Some(400));
    assert_eq!(err.code.as_deref(), Some("unsupported_chataibot_mask"));
}

#[test]
fn chataibot_non_success_plain_text_is_classified_before_json_decode() {
    let err = parse_chataibot_http_response(
        401,
        Some("text/plain"),
        "Unauthorized",
        "chataibot_compatible",
    )
    .expect_err("plain-text 401 must be classified as an upstream error");

    assert_eq!(err.kind, ErrorKind::Authentication);
    assert_eq!(err.http_status, Some(401));
    assert_eq!(err.message, "Unauthorized");
}

#[test]
fn chataibot_empty_success_has_explicit_error_code() {
    let err =
        parse_chataibot_http_response(204, Some("application/json"), "  ", "chataibot_compatible")
            .expect_err("an empty successful response must not be accepted");

    assert_eq!(err.code.as_deref(), Some("chataibot_empty_response"));
    assert_eq!(err.provider_name.as_deref(), Some("chataibot_compatible"));
}

#[test]
fn chataibot_non_json_success_has_explicit_error_code_and_safe_preview() {
    let err = parse_chataibot_http_response(
        200,
        Some("text/html; charset=utf-8"),
        "<html> token=secret-value </html>",
        "chataibot_compatible",
    )
    .expect_err("a non-JSON successful response must not be accepted");

    assert_eq!(err.code.as_deref(), Some("chataibot_invalid_response"));
    assert!(err.message.contains("text/html; charset=utf-8"));
    assert!(!err.message.contains("secret-value"));
}

#[test]
fn chataibot_valid_json_success_is_preserved() {
    let body = parse_chataibot_http_response(
        200,
        Some("application/json"),
        r#"{"imageUrl":"https://example.com/paris.png"}"#,
        "chataibot_compatible",
    )
    .expect("valid JSON response should pass");

    assert_eq!(
        body.get("imageUrl").and_then(Value::as_str),
        Some("https://example.com/paris.png")
    );
}
