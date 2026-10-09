mod media_outputs;
mod planning;
mod upstream_errors;
mod worker_results;
use super::*;
use crate::routing::candidate::ProviderAccountPayload;

fn image_generation_request(
    raw_body: serde_json::Value,
) -> crate::protocol::canonical::CanonicalRelayRequest {
    crate::protocol::canonical::CanonicalRelayRequest {
        protocol_family: crate::protocol::canonical::ProtocolFamily::OpenAi,
        endpoint_kind: crate::protocol::canonical::EndpointKind::ImagesGenerations,
        requested_model: None,
        stream: false,
        messages: Vec::new(),
        tools: Vec::new(),
        tool_choice: None,
        reasoning: None,
        metadata: None,
        raw_body,
        previous_response_id: None,
        explicit_session_key: None,
        extra: std::collections::HashMap::new(),
    }
}

fn music_generation_request(
    raw_body: serde_json::Value,
) -> crate::protocol::canonical::CanonicalRelayRequest {
    crate::protocol::canonical::CanonicalRelayRequest {
        protocol_family: crate::protocol::canonical::ProtocolFamily::OpenAi,
        endpoint_kind: crate::protocol::canonical::EndpointKind::MusicGenerations,
        requested_model: None,
        stream: false,
        messages: Vec::new(),
        tools: Vec::new(),
        tool_choice: None,
        reasoning: None,
        metadata: None,
        raw_body,
        previous_response_id: None,
        explicit_session_key: None,
        extra: std::collections::HashMap::new(),
    }
}

fn make_payload(base_url: &str) -> ProviderAccountPayload {
    ProviderAccountPayload {
        discovered_protocols: Vec::new(),
        adapter: "udio_compatible".to_string(),
        base_url: base_url.to_string(),
        api_key: "sk-test".to_string(),
        credential_id: None,
        expires_at: None,
        runtime_state_object_key: None,
        account_name: None,
        execution_mode: None,
        endpoint_execution_modes: None,
        default_model: None,
        headers: std::collections::HashMap::new(),
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
