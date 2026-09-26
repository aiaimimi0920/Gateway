use std::collections::HashMap;
use std::time::Duration;

use crate::protocol::suno;
use rquest::header::{HeaderMap, HeaderValue};
use rquest::Method;
use serde_json::json;

use super::{
    build_suno_browser_executor_payload, build_suno_browser_worker_input,
    build_suno_challenge_check_plan, build_suno_feed_poll_plan, build_suno_generate_plan,
    build_suno_runtime_headers, missing_runtime_bearer_error, missing_runtime_cookie_error,
    prepare_suno_browser_executor_service_input, prepare_suno_execution_context,
    prepare_suno_execution_plan, prepare_suno_request_context, suno_user_tier,
    unsupported_request_plan_error, unsupported_suno_edit_endpoint_error,
    unsupported_suno_image_inputs_error, unsupported_suno_media_generation_endpoint_error,
    unsupported_suno_video_count_error, validate_suno_media_request,
};
use crate::protocol::canonical::{CanonicalRelayRequest, EndpointKind, ProtocolFamily};
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::common::RequestPlan;

fn make_request(endpoint_kind: EndpointKind) -> CanonicalRelayRequest {
    CanonicalRelayRequest {
        protocol_family: ProtocolFamily::OpenAi,
        endpoint_kind,
        requested_model: Some("chirp-v3-5".to_string()),
        stream: false,
        messages: Vec::new(),
        tools: Vec::new(),
        tool_choice: None,
        reasoning: None,
        metadata: None,
        raw_body: json!({}),
        previous_response_id: None,
        explicit_session_key: None,
        extra: HashMap::new(),
    }
}

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

fn assert_suno_request_plan(
    plan: &RequestPlan,
    expected_url: &str,
    expected_response_kind: EndpointKind,
) {
    assert_eq!(plan.method, Method::POST);
    assert_eq!(plan.url, expected_url);
    assert_eq!(plan.response_kind, expected_response_kind);
    assert!(plan.query.is_empty());
}

mod execution;
mod plans;
