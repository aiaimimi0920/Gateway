use super::*;
use std::collections::HashMap;

use rquest::Method;
use serde_json::json;

pub(super) fn make_payload(adapter: &str, base_url: &str) -> ProviderAccountPayload {
    ProviderAccountPayload {
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

pub(super) fn make_request(
    protocol: ProtocolFamily,
    endpoint: EndpointKind,
) -> CanonicalRelayRequest {
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

pub(super) fn make_runtime() -> GeminiBusinessRuntime {
    GeminiBusinessRuntime {
        config_id: "cfg-123".to_string(),
        session: "projects/demo/sessions/abc".to_string(),
        language_code: "en-US".to_string(),
        time_zone: "UTC".to_string(),
        answer_generation_mode: "NORMAL".to_string(),
        assist_skipping_mode: "REQUEST_ASSIST".to_string(),
        additional_token: "-".to_string(),
        stream_assist_path: "/locations/global/widgetStreamAssist".to_string(),
        context_file_upload_path: "/locations/global/widgetAddContextFile".to_string(),
    }
}

pub(super) fn make_upload() -> GeminiBusinessUpload {
    GeminiBusinessUpload {
        mime_type: "image/png".to_string(),
        base64_data: "aGVsbG8=".to_string(),
    }
}

pub(super) fn assert_request_plan(
    plan: &RequestPlan,
    expected_url: &str,
    expected_response_kind: EndpointKind,
) {
    assert_eq!(plan.method, Method::POST);
    assert_eq!(plan.url, expected_url);
    assert_eq!(plan.response_kind, expected_response_kind);
    assert!(plan.query.is_empty());
}
