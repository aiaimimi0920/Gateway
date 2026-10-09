mod media_outputs;
mod planning;
mod worker_results;
use super::{
    build_lumalabs_browser_executor_service_result, build_lumalabs_downloaded_image_response,
    build_lumalabs_non_image_generation_response, classify_lumalabs_browser_worker_failure,
    classify_lumalabs_media_fetch_error, ensure_successful_lumalabs_media_fetch_status,
    extract_lumalabs_browser_worker_success, parse_lumalabs_browser_worker_output,
    parse_lumalabs_browser_worker_verified_output,
    parse_lumalabs_remote_browser_executor_signed_url, prepare_lumalabs_browser_execution_input,
    prepare_lumalabs_execution_context, prepare_lumalabs_media_plan,
    resolve_lumalabs_browser_worker_result, resolve_lumalabs_downloaded_image_mime_type,
    resolve_lumalabs_image_generation_plan,
};
use crate::routing::candidate::ProviderAccountPayload;
use std::collections::HashMap;
use std::time::Duration;

fn make_payload(adapter: &str, base_url: &str) -> ProviderAccountPayload {
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
        extra_body: Some(HashMap::from([(
            "realmId".to_string(),
            serde_json::json!("realm-123"),
        )])),
        session_auth: None,
        keepalive: None,
    }
}

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
