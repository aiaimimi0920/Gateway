use super::*;

use std::collections::HashMap;
use std::future::Future;

use crate::protocol::canonical::{
    CanonicalMessage, CanonicalRelayRequest, ContentPart, EndpointKind, MessageRole, ProtocolFamily,
};
use crate::upstream::header_map_helpers::header_map_string;

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

fn make_video_request() -> CanonicalRelayRequest {
    CanonicalRelayRequest {
        protocol_family: ProtocolFamily::OpenAi,
        endpoint_kind: EndpointKind::VideosGenerations,
        requested_model: Some("veo".to_string()),
        stream: false,
        messages: vec![CanonicalMessage {
            role: MessageRole::User,
            content: vec![ContentPart::Text {
                text: "generate a short video".to_string(),
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

fn make_music_request() -> CanonicalRelayRequest {
    CanonicalRelayRequest {
        protocol_family: ProtocolFamily::OpenAi,
        endpoint_kind: EndpointKind::MusicGenerations,
        requested_model: Some("lyria".to_string()),
        stream: false,
        messages: vec![CanonicalMessage {
            role: MessageRole::User,
            content: vec![ContentPart::Text {
                text: "generate a short music cue".to_string(),
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

#[test]
fn parse_gemini_canvas_official_json_body_reports_invalid_json() {
    let error =
        parse_gemini_canvas_official_json_body("{not-json").expect_err("invalid JSON should fail");
    assert_eq!(error.http_status, Some(500));
    assert_eq!(
        error.provider_name.as_deref(),
        Some("gemini_canvas_compatible")
    );
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_official_invalid_json")
    );
    assert!(error
        .message
        .starts_with("Gemini Canvas official API returned invalid JSON:"));
}

#[test]
fn gemini_canvas_official_api_enabled_requires_non_blank_api_key() {
    let mut payload = make_payload("gemini_canvas_compatible", "https://gemini.google.com");
    payload.api_key = "  ".to_string();
    assert!(!gemini_canvas_official_api_enabled(&payload));

    payload.api_key = "AIza-test".to_string();
    assert!(gemini_canvas_official_api_enabled(&payload));
}

#[test]
fn build_gemini_canvas_official_headers_uses_gemini_api_header_contract() {
    let payload = make_payload("gemini_canvas_compatible", "https://gemini.google.com");
    let extra_headers = HashMap::from([
        ("x-demo".to_string(), "demo".to_string()),
        (
            "Authorization".to_string(),
            "Bearer should-be-ignored".to_string(),
        ),
    ]);

    let headers = build_gemini_canvas_official_headers(&payload, Some(&extra_headers));

    assert_eq!(
        header_map_string(&headers, "x-goog-api-key").as_deref(),
        Some("sk-test")
    );
    assert_eq!(
        header_map_string(&headers, "x-demo").as_deref(),
        Some("demo")
    );
    assert_eq!(header_map_string(&headers, "authorization"), None);
}

#[test]
fn send_gemini_canvas_official_json_returns_json_value_result() {
    fn assert_future_output<F>(_future: F)
    where
        F: Future<Output = Result<Value, GatewayError>>,
    {
    }

    let http = rquest::Client::new();
    let payload = make_payload(
        "gemini_canvas_compatible",
        "https://generativelanguage.googleapis.com",
    );
    let body = serde_json::json!({"instances": [{"prompt": "generate a short video"}]});

    assert_future_output(send_gemini_canvas_official_json(
        &http,
        &payload,
        "https://generativelanguage.googleapis.com/v1beta/models/veo:predictLongRunning",
        &body,
        std::time::Duration::from_secs(1),
        None,
    ));
}

#[test]
fn send_gemini_canvas_official_get_json_returns_json_value_result() {
    fn assert_future_output<F>(_future: F)
    where
        F: Future<Output = Result<Value, GatewayError>>,
    {
    }

    let http = rquest::Client::new();
    let payload = make_payload(
        "gemini_canvas_compatible",
        "https://generativelanguage.googleapis.com",
    );

    assert_future_output(send_gemini_canvas_official_get_json(
        &http,
        &payload,
        "https://generativelanguage.googleapis.com/v1beta/operations/video-123",
        std::time::Duration::from_secs(1),
        None,
    ));
}

#[test]
fn send_gemini_canvas_official_get_bytes_returns_bytes_and_content_type_result() {
    fn assert_future_output<F>(_future: F)
    where
        F: Future<Output = Result<(bytes::Bytes, Option<String>), GatewayError>>,
    {
    }

    let http = rquest::Client::new();
    let payload = make_payload(
        "gemini_canvas_compatible",
        "https://generativelanguage.googleapis.com",
    );

    assert_future_output(send_gemini_canvas_official_get_bytes(
        &http,
        &payload,
        "https://generativelanguage.googleapis.com/v1beta/files/file-123",
        std::time::Duration::from_secs(1),
        None,
    ));
}

#[test]
fn connect_gemini_canvas_music_socket_returns_websocket_result() {
    fn assert_future_output<F>(_future: F)
    where
        F: Future<
            Output = Result<
                tokio_tungstenite::WebSocketStream<
                    tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
                >,
                GatewayError,
            >,
        >,
    {
    }

    let payload = make_payload(
        "gemini_canvas_compatible",
        "https://generativelanguage.googleapis.com",
    );

    assert_future_output(connect_gemini_canvas_music_socket(
        &payload,
        None,
        None,
        Some("wss://generativelanguage.googleapis.com/ws/test"),
    ));
}

#[test]
fn execute_gemini_canvas_official_video_returns_json_value_result() {
    fn assert_future_output<F>(_future: F)
    where
        F: Future<Output = Result<Value, GatewayError>>,
    {
    }

    let http = rquest::Client::new();
    let payload = make_payload(
        "gemini_canvas_compatible",
        "https://generativelanguage.googleapis.com/v1beta",
    );
    let req = make_video_request();

    assert_future_output(execute_gemini_canvas_official_video(
        &http,
        std::time::Duration::from_secs(1),
        &payload,
        &req,
        "veo",
        None,
        Some("https://generativelanguage.googleapis.com/v1beta/models/veo:predictLongRunning"),
        Some("generate a short video"),
        Some("16:9"),
        Some(8.0),
        Some("veo"),
    ));
}

#[test]
fn execute_gemini_canvas_official_music_returns_json_value_result() {
    fn assert_future_output<F>(_future: F)
    where
        F: Future<Output = Result<Value, GatewayError>>,
    {
    }

    let payload = make_payload(
        "gemini_canvas_compatible",
        "https://generativelanguage.googleapis.com/v1beta",
    );
    let req = make_music_request();

    assert_future_output(execute_gemini_canvas_official_music(
        std::time::Duration::from_secs(1),
        &payload,
        &req,
        "lyria",
        None,
        None,
        Some("wss://generativelanguage.googleapis.com/ws/test"),
        Some("generate a short music cue"),
        None,
        Some("lyria-realtime-exp"),
    ));
}
