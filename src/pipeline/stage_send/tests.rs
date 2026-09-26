use super::*;
use crate::protocol::canonical::CanonicalRelayRequest;
use crate::protocol::canonical::{
    CanonicalRelayResponse, CanonicalTool, EndpointKind, ProtocolFamily, TokenUsage,
};
use crate::routing::candidate::{ProviderAccountPayload, ProviderExecutionMode, RouteCandidate};
use std::collections::HashMap;

mod dispatch;
mod metrics;
mod packing;
mod policy;
mod selection;

fn make_req(protocol: ProtocolFamily, endpoint: EndpointKind) -> CanonicalRelayRequest {
    use crate::protocol::canonical::{CanonicalMessage, ContentPart, MessageRole};
    CanonicalRelayRequest {
        protocol_family: protocol,
        endpoint_kind: endpoint,
        requested_model: Some("gpt-4o".to_string()),
        stream: false,
        messages: vec![CanonicalMessage {
            role: MessageRole::User,
            content: vec![ContentPart::Text {
                text: "hi".to_string(),
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
    }
}

fn make_resp() -> CanonicalRelayResponse {
    CanonicalRelayResponse {
        model: "gpt-4o".to_string(),
        text: "Hello!".to_string(),
        usage: Some(TokenUsage {
            prompt_tokens: 5,
            completion_tokens: 3,
            total_tokens: 8,
            cache_creation_input_tokens: None,
            cache_read_input_tokens: None,
        }),
        tool_calls: vec![],
        upstream_status: Some(200),
        finish_reason: Some("stop".to_string()),
    }
}

fn make_candidate(adapter: &str) -> RouteCandidate {
    RouteCandidate {
        provider_account_id: "prov-1".to_string(),
        provider_credential_id: Some("cred-1".to_string()),
        label: "Provider".to_string(),
        payload: ProviderAccountPayload {
            adapter: adapter.to_string(),
            base_url: "https://api.example.com".to_string(),
            api_key: "sk-test".to_string(),
            credential_id: Some("cred-ref".to_string()),
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
        },
        protocol_family: "openai".to_string(),
        protocol_profile: "openai".to_string(),
        supported_protocol_families: vec!["openai_chat".to_string()],
        adapter: adapter.to_string(),
        model_alias: Some("gpt-4o".to_string()),
        upstream_model: Some("gpt-4o".to_string()),
        resolved_execution_mode: ProviderExecutionMode::DirectHttp,
        priority: 100,
        weight: 1,
        failure_count: 0,
        cooldown_until: None,
        routing_score: Some(0.95),
        routing_health_weight: Some(0.9),
        routing_capacity_weight: Some(1.0),
        routing_degraded: Some(false),
        routing_breaker_open: Some(false),
        routing_degradation_reasons: Vec::new(),
    }
}
