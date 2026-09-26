use super::*;
use crate::protocol::canonical::{CanonicalRelayRequest, EndpointKind, ProtocolFamily};
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::client::UpstreamClient;
use rquest::Method;
use std::collections::HashMap;

fn make_payload(adapter: &str, base_url: &str) -> ProviderAccountPayload {
    ProviderAccountPayload {
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

fn assert_request_plan(
    plan: &crate::upstream::common::RequestPlan,
    expected_url: &str,
    expected_response_kind: EndpointKind,
) {
    assert_eq!(plan.method, Method::POST);
    assert_eq!(plan.url, expected_url);
    assert_eq!(plan.response_kind, expected_response_kind);
    assert!(plan.query.is_empty());
}

#[test]
fn producer_unsupported_request_plan_error_matches_contract() {
    let err = unsupported_request_plan_error();
    assert_eq!(err.http_status, Some(400));
    assert_eq!(err.provider_name.as_deref(), Some("producer_compatible"));
    assert_eq!(err.code.as_deref(), Some("unsupported_producer_endpoint"));
}

#[test]
fn plan_producer_chat_endpoint_rejected_locally() {
    let payload = make_payload("producer_compatible", "https://www.producer.ai");
    let req = make_request(ProtocolFamily::OpenAi, EndpointKind::ChatCompletions);
    let err = UpstreamClient::build_request_plan(&payload, &req, PRODUCER_DEFAULT_MODEL, false)
        .expect_err("producer chat requests should be rejected");
    assert_eq!(err.http_status, Some(400));
    assert_eq!(err.code.as_deref(), Some("unsupported_producer_endpoint"));
}

#[test]
fn producer_unsupported_media_endpoint_error_matches_contract() {
    let err = unsupported_media_endpoint_error();
    assert_eq!(err.http_status, Some(400));
    assert_eq!(err.provider_name.as_deref(), Some("producer_compatible"));
    assert_eq!(err.code.as_deref(), Some("unsupported_producer_endpoint"));
}

#[test]
fn build_send_message_plan_uses_conversation_endpoint() {
    let payload = make_payload("producer_compatible", "https://producer.ai/");
    let req = normalize_music_generations(json!({
        "prompt": "lo-fi piano with rain ambience"
    }))
    .unwrap();

    let plan = build_send_message_plan(&payload, &req, PRODUCER_DEFAULT_MODEL)
        .expect("producer send message plan");
    assert_request_plan(
        &plan,
        "https://producer.ai/__api/conversation",
        EndpointKind::MusicGenerations,
    );
    assert_eq!(
        plan.body.as_ref().expect("producer send message plan body"),
        &build_send_message_request(&req, PRODUCER_DEFAULT_MODEL)
            .expect("producer send message body"),
    );
}
