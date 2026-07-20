use crate::protocol::canonical::{
    CanonicalRelayRequest, CanonicalRelayResponse, CanonicalToolCall, EndpointKind, ProtocolFamily,
    TokenUsage,
};
use crate::upstream::canonical_sse::canonical_response_to_openai_sse_bytes;
use serde_json::{json, Value};
use std::collections::HashMap;

fn minimal_request(requested_model: Option<&str>) -> CanonicalRelayRequest {
    CanonicalRelayRequest {
        protocol_family: ProtocolFamily::OpenAi,
        endpoint_kind: EndpointKind::ChatCompletions,
        requested_model: requested_model.map(str::to_string),
        stream: true,
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

fn sse_payloads(frames: Vec<bytes::Bytes>) -> Vec<String> {
    frames
        .into_iter()
        .map(|frame| String::from_utf8(frame.to_vec()).expect("SSE frame is UTF-8"))
        .map(|frame| {
            frame
                .strip_prefix("data: ")
                .expect("OpenAI SSE frame uses data prefix")
                .trim_end()
                .to_string()
        })
        .collect()
}

fn parse_json_payload(payload: &str) -> Value {
    serde_json::from_str(payload).expect("SSE payload is JSON")
}

#[test]
fn canonical_sse_prefers_requested_model_and_emits_usage_and_done() {
    let req = minimal_request(Some("caller-model"));
    let response = CanonicalRelayResponse {
        model: "upstream-model".to_string(),
        text: "hello".to_string(),
        usage: Some(TokenUsage {
            prompt_tokens: 1,
            completion_tokens: 2,
            total_tokens: 3,
            ..Default::default()
        }),
        tool_calls: Vec::new(),
        upstream_status: Some(200),
        finish_reason: None,
    };

    let payloads = sse_payloads(canonical_response_to_openai_sse_bytes(
        &req,
        "fallback-model",
        &response,
    ));

    assert_eq!(payloads.last().map(String::as_str), Some("[DONE]"));
    let text_frame = parse_json_payload(&payloads[0]);
    assert_eq!(text_frame["model"], "caller-model");
    assert_eq!(text_frame["choices"][0]["delta"]["content"], "hello");
    let stop_frame = parse_json_payload(&payloads[payloads.len() - 2]);
    assert_eq!(stop_frame["model"], "caller-model");
    assert_eq!(stop_frame["choices"][0]["finish_reason"], "stop");
    assert_eq!(stop_frame["usage"]["prompt_tokens"], 1);
    assert_eq!(stop_frame["usage"]["completion_tokens"], 2);
    assert_eq!(stop_frame["usage"]["total_tokens"], 3);
}

#[test]
fn canonical_sse_streams_tool_call_start_arguments_and_tool_finish() {
    let req = minimal_request(None);
    let response = CanonicalRelayResponse {
        model: "upstream-model".to_string(),
        text: String::new(),
        usage: None,
        tool_calls: vec![CanonicalToolCall {
            id: Some("call_1".to_string()),
            call_type: "function".to_string(),
            name: Some("weather".to_string()),
            arguments: Some(r#"{"city":"Hangzhou"}"#.to_string()),
            raw: HashMap::new(),
        }],
        upstream_status: Some(200),
        finish_reason: None,
    };

    let payloads = sse_payloads(canonical_response_to_openai_sse_bytes(
        &req,
        "fallback-model",
        &response,
    ));

    assert_eq!(payloads.last().map(String::as_str), Some("[DONE]"));
    let tool_start = parse_json_payload(&payloads[0]);
    let started_call = &tool_start["choices"][0]["delta"]["tool_calls"][0];
    assert_eq!(tool_start["model"], "fallback-model");
    assert_eq!(started_call["id"], "call_1");
    assert_eq!(started_call["type"], "function");
    assert_eq!(started_call["function"]["name"], "weather");
    assert_eq!(started_call["function"]["arguments"], "");

    let tool_arguments = parse_json_payload(&payloads[1]);
    assert_eq!(
        tool_arguments["choices"][0]["delta"]["tool_calls"][0]["function"]["arguments"],
        r#"{"city":"Hangzhou"}"#
    );

    let stop_frame = parse_json_payload(&payloads[payloads.len() - 2]);
    assert_eq!(stop_frame["choices"][0]["finish_reason"], "tool_calls");
    assert!(stop_frame["usage"].is_null());
}
