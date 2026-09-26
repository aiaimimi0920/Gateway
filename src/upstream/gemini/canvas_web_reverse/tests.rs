use base64::Engine;
use serde_json::json;

use super::*;
use crate::error::GatewayError;
use crate::protocol::canonical::{CanonicalRelayRequest, EndpointKind, ProtocolFamily};
use crate::protocol::gemini::shared::GEMINI_CANVAS_WEB_REVERSE_MODULAR_ADAPTER;
use crate::protocol::gemini_canvas;
use crate::routing::candidate::ProviderAccountPayload;

fn make_payload() -> ProviderAccountPayload {
    ProviderAccountPayload {
        adapter: GEMINI_CANVAS_WEB_REVERSE_MODULAR_ADAPTER.to_string(),
        base_url: "https://gemini.google.com".to_string(),
        api_key: "unused".to_string(),
        credential_id: None,
        expires_at: None,
        runtime_state_object_key: Some(
            "credential-runtime/gemini-canvas/browser/storage-state.json".to_string(),
        ),
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
        extra_body: Some(std::collections::HashMap::from([(
            "shareId".to_string(),
            json!("canvas-share-789"),
        )])),
        session_auth: None,
        keepalive: None,
    }
}

fn make_media_result(
    media: Vec<GeminiCanvasBrowserOwnedMediaAsset>,
) -> crate::upstream::gemini::canvas_program_web_reverse::GeminiCanvasBrowserInvocationResult {
    GeminiCanvasBrowserOwnedInvocationResult {
        operation: "image".to_string(),
        share_url: None,
        share_id: None,
        share_follow_kind: None,
        before_url: None,
        final_url: None,
        page_url: None,
        canvas_program_url: None,
        app_path: None,
        conversation_id: None,
        response_id: None,
        last_seen_conversation_id: None,
        last_seen_response_id: None,
        candidate_pairs: Vec::new(),
        stable_program_pair: None,
        latest_response_pair: None,
        aggregate_hints: None,
        captured_at: None,
        last_validated_at: None,
        new_chat_clicked: None,
        mode_selected: None,
        body_text: None,
        body_base64: None,
        mime_type: None,
        text: None,
        media,
    }
    .into()
}

fn make_media_asset(kind: &str, url: &str, mime_type: &str) -> GeminiCanvasBrowserOwnedMediaAsset {
    GeminiCanvasBrowserOwnedMediaAsset {
        kind: kind.to_string(),
        url: url.to_string(),
        mime_type: mime_type.to_string(),
        body_base64: None,
        alt: None,
        width: None,
        height: None,
        duration_seconds: None,
    }
}

fn make_converted_image_asset(url: &str, mime_type: &str) -> gemini_canvas::GeminiCanvasMediaAsset {
    gemini_canvas::GeminiCanvasMediaAsset {
        kind: "image".to_string(),
        url: url.to_string(),
        mime_type: mime_type.to_string(),
        download_token: None,
        body_base64: None,
        alt: None,
        width: None,
        height: None,
        duration_seconds: None,
    }
}

fn make_media_request(endpoint_kind: EndpointKind) -> CanonicalRelayRequest {
    CanonicalRelayRequest {
        protocol_family: ProtocolFamily::OpenAi,
        endpoint_kind,
        requested_model: None,
        stream: false,
        messages: Vec::new(),
        tools: Vec::new(),
        tool_choice: None,
        reasoning: None,
        metadata: None,
        raw_body: json!({}),
        previous_response_id: None,
        explicit_session_key: None,
        extra: std::collections::HashMap::new(),
    }
}

mod audio_contracts;
mod input_contracts;
mod media_result_contracts;
mod response_contracts;
