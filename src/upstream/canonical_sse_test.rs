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
        sources: Vec::new(),
        bedrock_turn: None,
        vertex_turn: None,
        model_declaration: None,
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
        sources: Vec::new(),
        bedrock_turn: None,
        vertex_turn: None,
        model_declaration: None,
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

#[test]
fn response_sources_canonical_sse_emits_final_list_once_without_changing_answer() {
    let sources = json!([{"name":"标题", "url":"https://example.org", "snippet":"摘要"}]);
    let response: CanonicalRelayResponse = serde_json::from_value(json!({
        "model":"unknown", "text":"答案 [1]", "usage":null, "tool_calls":[],
        "upstream_status":200, "finish_reason":"stop", "sources":sources,
    }))
    .unwrap();
    let frames = sse_payloads(canonical_response_to_openai_sse_bytes(
        &minimal_request(Some("caller-alias")),
        "unknown",
        &response,
    ));
    assert_eq!(frames.last().unwrap(), "[DONE]");
    let text = parse_json_payload(&frames[0]);
    assert_eq!(text["choices"][0]["delta"]["content"], "答案 [1]");
    assert!(text.get("gateway_sources").is_none());
    let stop = parse_json_payload(&frames[frames.len() - 2]);
    assert_eq!(stop["gateway_sources"], sources);
    assert_eq!(stop["model"], "caller-alias");
    assert!(stop["usage"].is_null());
    assert!(stop.get("annotations").is_none());
}

#[tokio::test]
async fn response_sources_perplexity_final_snapshot_reaches_responses_aggregate() {
    use crate::protocol::{perplexity_web::Decoder, responses};
    use futures::{stream, StreamExt};
    let sources = json!([{"name":"标题", "url":"https://example.org", "snippet":"摘要"}]);
    let raw = json!({"blocks":[
        {"intended_usage":"ask_text", "markdown_block":{"answer":"答案 [1]"}},
        {"intended_usage":"web_results", "web_result_block":{"web_results":sources}}
    ], "read_write_token":"must-not-project"});
    let wire = format!("data: {}\n\nevent: end_of_stream\ndata: {{}}\n\n", raw);
    let mut decoder = Decoder::new();
    let canonical = decoder
        .push(bytes::Bytes::from(wire))
        .unwrap()
        .unwrap()
        .into_canonical();
    decoder.finish().unwrap();
    assert_eq!(canonical.model, "unknown");
    assert!(canonical.model_declaration.is_none());
    assert!(canonical.usage.is_none());
    let input = canonical_response_to_openai_sse_bytes(
        &minimal_request(Some("caller-alias")),
        "unknown",
        &canonical,
    )
    .into_iter()
    .map(Ok::<_, crate::protocol::stream_error::ProtocolStreamError>);
    let output = responses::translate_openai_sse_to_responses_with_error(
        stream::iter(input),
        "unknown".into(),
    );
    let bytes: Vec<_> = output
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .flat_map(|part| part.unwrap().to_vec())
        .collect();
    assert!(!String::from_utf8_lossy(&bytes).contains("must-not-project"));
    let input = stream::iter(vec![Ok::<
        _,
        crate::protocol::stream_error::ProtocolStreamError,
    >(bytes::Bytes::from(bytes))]);
    let result = responses::accumulate_responses_sse_stream_with_error(Box::pin(input), "unknown")
        .await
        .unwrap();
    assert_eq!(result.text, "答案 [1]");
    assert!(result.usage.is_none());
    assert_eq!(serde_json::to_value(result).unwrap()["sources"], sources);
}
