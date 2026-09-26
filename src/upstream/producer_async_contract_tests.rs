use super::*;
use crate::protocol::canonical::{
    CanonicalMessage, CanonicalRelayRequest, ContentPart, EndpointKind, MessageRole, ProtocolFamily,
};
use crate::protocol::producer::{PRODUCER_DEFAULT_MODEL, PRODUCER_IMAGE_DEFAULT_MODEL};
use std::collections::HashMap;
use std::future::Future;

#[test]
fn send_producer_conversation_returns_shared_job_data_type() {
    fn assert_future_output<F>(_future: F)
    where
        F: Future<Output = Result<ProducerConversationJobData, GatewayError>>,
    {
    }

    let http = rquest::Client::new();
    let headers = HeaderMap::new();
    let client_context = json!({});
    assert_future_output(send_producer_conversation(
        &http,
        "https://www.flowmusic.app",
        &headers,
        "make a video",
        None,
        &client_context,
        crate::protocol::producer::PRODUCER_DEFAULT_MODEL,
        "https://www.flowmusic.app/",
        std::time::Duration::from_secs(1),
    ));
}

#[test]
fn read_producer_message_stream_returns_string_result() {
    fn assert_future_output<F>(_future: F)
    where
        F: Future<Output = Result<String, GatewayError>>,
    {
    }

    let http = rquest::Client::new();
    let headers = HeaderMap::new();
    assert_future_output(read_producer_message_stream(
        &http,
        "https://www.flowmusic.app",
        &headers,
        "job-123",
        "https://www.flowmusic.app/",
        std::time::Duration::from_secs(1),
    ));
}

#[test]
fn fetch_producer_video_status_returns_json_value_result() {
    fn assert_future_output<F>(_future: F)
    where
        F: Future<Output = Result<serde_json::Value, GatewayError>>,
    {
    }

    let http = rquest::Client::new();
    let headers = HeaderMap::new();
    assert_future_output(fetch_producer_video_status(
        &http,
        "https://www.flowmusic.app",
        &headers,
        "video-job-123",
        "https://www.flowmusic.app/",
        std::time::Duration::from_secs(1),
    ));
}

#[test]
fn execute_producer_browser_worker_returns_json_value_result() {
    fn assert_future_output<F>(_future: F)
    where
        F: Future<Output = Result<serde_json::Value, GatewayError>>,
    {
    }

    let prepared = PreparedProducerBrowserExecutorServiceInput {
        base_url: "https://www.flowmusic.app".to_string(),
        headers: HeaderMap::new(),
        request_body: json!({"prompt": "make a video"}),
        model: PRODUCER_IMAGE_DEFAULT_MODEL.to_string(),
        timeout: std::time::Duration::from_secs(1),
    };

    assert_future_output(execute_producer_browser_worker(
        "producer_compatible",
        &prepared,
        false,
    ));
}

#[test]
fn execute_producer_video_http_returns_json_value_result() {
    fn assert_future_output<F>(_future: F)
    where
        F: Future<Output = Result<serde_json::Value, GatewayError>>,
    {
    }

    let http = rquest::Client::new();
    let headers = HeaderMap::new();
    let request_body = json!({"prompt": "make a video"});
    assert_future_output(execute_producer_video_http(
        &http,
        "https://www.flowmusic.app",
        &headers,
        &request_body,
        "producer-video",
        std::time::Duration::from_secs(1),
    ));
}

#[test]
fn execute_producer_image_http_returns_json_value_result() {
    fn assert_future_output<F>(_future: F)
    where
        F: Future<Output = Result<serde_json::Value, GatewayError>>,
    {
    }

    let http = rquest::Client::new();
    let headers = HeaderMap::new();
    let req = CanonicalRelayRequest {
        protocol_family: ProtocolFamily::OpenAi,
        endpoint_kind: EndpointKind::ImagesGenerations,
        requested_model: Some(PRODUCER_IMAGE_DEFAULT_MODEL.to_string()),
        stream: false,
        messages: vec![CanonicalMessage {
            role: MessageRole::User,
            content: vec![ContentPart::Text {
                text: "make an image".to_string(),
            }],
            name: None,
            tool_call_id: None,
            tool_calls: vec![],
        }],
        tools: vec![],
        tool_choice: None,
        reasoning: None,
        metadata: None,
        raw_body: json!({"prompt": "make an image"}),
        previous_response_id: None,
        explicit_session_key: None,
        extra: HashMap::new(),
    };

    assert_future_output(execute_producer_image_http(
        &http,
        "https://www.flowmusic.app",
        &headers,
        &req,
        PRODUCER_IMAGE_DEFAULT_MODEL,
        std::time::Duration::from_secs(1),
    ));
}

#[test]
fn execute_producer_music_http_returns_json_value_result() {
    fn assert_future_output<F>(_future: F)
    where
        F: Future<Output = Result<serde_json::Value, GatewayError>>,
    {
    }

    let http = rquest::Client::new();
    let headers = HeaderMap::new();
    let req = CanonicalRelayRequest {
        protocol_family: ProtocolFamily::OpenAi,
        endpoint_kind: EndpointKind::MusicGenerations,
        requested_model: Some(PRODUCER_DEFAULT_MODEL.to_string()),
        stream: false,
        messages: vec![CanonicalMessage {
            role: MessageRole::User,
            content: vec![ContentPart::Text {
                text: "make a song".to_string(),
            }],
            name: None,
            tool_call_id: None,
            tool_calls: vec![],
        }],
        tools: vec![],
        tool_choice: None,
        reasoning: None,
        metadata: None,
        raw_body: json!({"prompt": "make a song"}),
        previous_response_id: None,
        explicit_session_key: None,
        extra: HashMap::new(),
    };

    assert_future_output(execute_producer_music_http(
        &http,
        "https://www.flowmusic.app",
        &headers,
        &req,
        PRODUCER_DEFAULT_MODEL,
        std::time::Duration::from_secs(1),
    ));
}
