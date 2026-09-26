mod candidate_selection;
mod gemini_bridges;
mod text_bridges;
use serde_json::json;

use crate::protocol::canonical::{
    CanonicalMessage, CanonicalRelayRequest, ContentPart, EndpointKind, MessageRole, ProtocolFamily,
};
use crate::routing::candidate::{ProviderAccountPayload, ProviderExecutionMode, RouteCandidate};

use super::request_compatibility::request_compatible_wire_protocol_families;
use super::*;
use crate::protocol::registry::{
    ANTHROPIC_MESSAGES_FAMILY, CHATGPT_WEB_CHAT_FAMILY, GEMINI_CANVAS_IMAGES_FAMILY,
    GEMINI_CANVAS_MUSIC_FAMILY, GEMINI_CANVAS_VIDEOS_FAMILY, GEMINI_GENERATE_CONTENT_FAMILY,
    LUMALABS_AUDIO_FAMILY, LUMALABS_IMAGES_FAMILY, LUMALABS_VIDEOS_FAMILY,
    OPENAI_AUDIO_SPEECH_FAMILY, OPENAI_CHAT_FAMILY, OPENAI_EMBEDDINGS_FAMILY,
    OPENAI_LEGACY_COMPLETIONS_FAMILY, OPENAI_RESPONSES_FAMILY, PRODUCER_IMAGES_FAMILY,
    PRODUCER_MUSIC_FAMILY, PRODUCER_VIDEOS_FAMILY, SUNO_IMAGES_FAMILY, SUNO_MUSIC_FAMILY,
    SUNO_VIDEOS_FAMILY, UDIO_IMAGES_FAMILY, UDIO_MUSIC_FAMILY, UDIO_VIDEOS_FAMILY,
};

fn make_request(
    protocol_family: ProtocolFamily,
    endpoint_kind: EndpointKind,
) -> CanonicalRelayRequest {
    CanonicalRelayRequest {
        protocol_family,
        endpoint_kind,
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
        raw_body: json!({}),
        previous_response_id: None,
        explicit_session_key: None,
        extra: Default::default(),
    }
}

fn make_candidate(protocol_family: &str) -> RouteCandidate {
    RouteCandidate {
        provider_account_id: "provider-1".to_string(),
        provider_credential_id: Some("cred-1".to_string()),
        label: "provider-1".to_string(),
        payload: ProviderAccountPayload {
            adapter: "openai_compatible".to_string(),
            base_url: "https://api.example.com".to_string(),
            api_key: "sk-test".to_string(),
            credential_id: None,
            expires_at: None,
            runtime_state_object_key: None,
            account_name: None,
            execution_mode: None,
            endpoint_execution_modes: None,
            default_model: None,
            headers: Default::default(),
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
        protocol_family: protocol_family.to_string(),
        protocol_profile: "openai".to_string(),
        supported_protocol_families: vec![
            OPENAI_CHAT_FAMILY.to_string(),
            OPENAI_RESPONSES_FAMILY.to_string(),
        ],
        adapter: "openai_compatible".to_string(),
        model_alias: Some("gpt-4o".to_string()),
        upstream_model: Some("gpt-4o".to_string()),
        resolved_execution_mode: ProviderExecutionMode::DirectHttp,
        priority: 10,
        weight: 1,
        failure_count: 0,
        cooldown_until: None,
        routing_score: None,
        routing_health_weight: None,
        routing_capacity_weight: None,
        routing_degraded: None,
        routing_breaker_open: None,
        routing_degradation_reasons: Vec::new(),
    }
}
