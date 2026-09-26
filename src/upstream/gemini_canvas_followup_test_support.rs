use crate::protocol::canonical::{
    CanonicalMessage, CanonicalRelayRequest, ContentPart, EndpointKind, MessageRole, ProtocolFamily,
};
use crate::protocol::gemini_web;
use crate::routing::candidate::ProviderAccountPayload;
use serde_json::json;
use std::collections::HashMap;

pub(super) fn make_payload(adapter: &str, base_url: &str) -> ProviderAccountPayload {
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

pub(super) fn make_request(
    protocol: ProtocolFamily,
    endpoint: EndpointKind,
) -> CanonicalRelayRequest {
    CanonicalRelayRequest {
        protocol_family: protocol,
        endpoint_kind: endpoint,
        requested_model: Some("gpt-4o".to_string()),
        stream: false,
        messages: vec![CanonicalMessage {
            role: MessageRole::User,
            content: vec![ContentPart::Text {
                text: "Hello".to_string(),
            }],
            name: None,
            tool_call_id: None,
            tool_calls: vec![],
        }],
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

pub(super) fn make_gemini_bootstrap() -> gemini_web::GeminiWebBootstrap {
    gemini_web::GeminiWebBootstrap {
        access_token: Some("at-test".to_string()),
        build_label: Some("boq-gemini".to_string()),
        session_id: Some("sid-test".to_string()),
        language: "en-US".to_string(),
        push_id: None,
        client_pctx: None,
        app_page_path: Some("/app/from-bootstrap".to_string()),
    }
}
