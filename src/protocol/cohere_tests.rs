use super::*;
use std::collections::HashMap;

fn base_request() -> CanonicalRelayRequest {
    CanonicalRelayRequest {
        protocol_family: ProtocolFamily::OpenAi,
        endpoint_kind: crate::protocol::canonical::EndpointKind::ChatCompletions,
        requested_model: Some("command-r".to_string()),
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
        raw_body: json!({}),
        previous_response_id: None,
        explicit_session_key: None,
        extra: HashMap::new(),
    }
}

#[test]
fn packs_tool_choice_required() {
    let mut req = base_request();
    req.tool_choice = Some(json!("required"));
    let body = pack_cohere(&req, "command-r", false);
    assert_eq!(body["tool_choice"], "REQUIRED");
}

#[test]
fn packs_assistant_tool_calls() {
    let mut req = base_request();
    req.messages.push(CanonicalMessage {
        role: MessageRole::Assistant,
        content: vec![],
        name: None,
        tool_call_id: None,
        tool_calls: vec![CanonicalToolCall {
            id: Some("call_1".to_string()),
            call_type: "function".to_string(),
            name: Some("weather".to_string()),
            arguments: Some("{\"city\":\"Hangzhou\"}".to_string()),
            raw: HashMap::new(),
        }],
    });
    let body = pack_cohere(&req, "command-r", false);
    assert_eq!(
        body["messages"][1]["tool_calls"][0]["function"]["name"],
        "weather"
    );
}

#[test]
fn cohere_stream_tool_calls_emit_single_object_start_and_end_frame() {
    let mut state = CohereStreamState::new("command-r".to_string());
    let sse = concat!(
        "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_1\",\"function\":{\"name\":\"weather\",\"arguments\":\"{\\\"city\\\":\"}}]}}]}\n\n",
        "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"\\\"Hangzhou\\\"}\"}}]}}]}\n\n",
        "data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"tool_calls\"}]}\n\n",
        "data: [DONE]\n\n"
    );
    state.buffer.extend_from_slice(sse.as_bytes());
    state.process_buffer();

    let rendered: String = state
        .outputs
        .iter()
        .map(|chunk| String::from_utf8_lossy(chunk).into_owned())
        .collect();
    assert_eq!(rendered.matches("event: tool-call-start\n").count(), 1);
    let start_frame = state
        .outputs
        .iter()
        .map(|chunk| String::from_utf8_lossy(chunk).into_owned())
        .find(|chunk| chunk.starts_with("event: tool-call-start\n"))
        .expect("tool-call-start frame");
    let start_json: Value = serde_json::from_str(
        start_frame
            .lines()
            .find_map(|line| line.strip_prefix("data: "))
            .expect("tool-call-start data line"),
    )
    .expect("valid start JSON");
    assert!(start_json["delta"]["message"]["tool_calls"].is_object());
    assert_eq!(start_json["delta"]["message"]["tool_calls"]["id"], "call_1");
    assert_eq!(
        start_json["delta"]["message"]["tool_calls"]["function"]["name"],
        "weather"
    );
    assert_eq!(
        start_json["delta"]["message"]["tool_calls"]["function"]["arguments"],
        ""
    );
    assert!(rendered.contains("event: tool-call-delta\n"));
    assert!(rendered.contains("event: tool-call-end\n"));
    assert!(rendered.contains("event: message-end\n"));
}

#[tokio::test]
async fn cohere_stream_ignores_null_usage_and_fuses_after_error() {
    use futures::StreamExt;
    let mut state = CohereStreamState::new("command-r".to_string());
    state.buffer.extend_from_slice(
        concat!(
            "data: {\"choices\":[{\"delta\":{\"content\":\"ok\"}}],\"usage\":{\"prompt_tokens\":3,\"completion_tokens\":2,\"total_tokens\":5}}\n\n",
            "data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}],\"usage\":null}\n\n",
            "data: {\"choices\":[],\"usage\":{}}\n\n",
            "data: [DONE]\n\n"
        )
        .as_bytes(),
    );
    state.process_buffer();
    let end_frame = state
        .outputs
        .iter()
        .map(|chunk| String::from_utf8_lossy(chunk).into_owned())
        .find(|chunk| chunk.starts_with("event: message-end\n"))
        .expect("message-end frame");
    let end_json: Value = serde_json::from_str(
        end_frame
            .lines()
            .find_map(|line| line.strip_prefix("data: "))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        end_json["delta"]["usage"],
        json!({"input_tokens": 3, "output_tokens": 2, "total_tokens": 5})
    );

    let chunks: Vec<Result<Bytes, &str>> = vec![
        Ok(Bytes::from_static(
            b"data: {\"choices\":[{\"delta\":{\"content\":\"hi\"}}]}\n\n",
        )),
        Err("boom"),
        Ok(Bytes::from_static(
            b"data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n",
        )),
    ];
    let mut stream = Box::pin(translate_openai_sse_to_cohere_stream_with_error(
        futures::stream::iter(chunks),
        "command-r".into(),
    ));
    assert!(stream.next().await.unwrap().is_ok());
    assert!(stream.next().await.unwrap().is_ok());
    assert_eq!(stream.next().await.unwrap().unwrap_err(), "boom");
    assert!(stream.next().await.is_none());
    assert!(stream.next().await.is_none());
}

#[tokio::test]
async fn cohere_stream_trailing_usage_and_clean_eof_do_not_repoll_source() {
    use crate::protocol::stream_error_test_support::probe;
    let input = concat!(
        "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"function\":{\"name\":\"lookup\"}}]}}]}\n\n",
        "data: {\"choices\":[{\"finish_reason\":\"tool_calls\"}]}\n\n",
        "data: {\"choices\":[],\"usage\":{\"prompt_tokens\":3,\"completion_tokens\":2}}\n\n"
    );
    let (source, stats) = probe(
        vec![Ok::<_, ()>(Bytes::from_static(input.as_bytes()))],
        false,
    );
    let out: Vec<_> = translate_openai_sse_to_cohere_stream_with_error(source, "command-r".into())
        .map(|chunk| String::from_utf8(chunk.unwrap().to_vec()).unwrap())
        .collect()
        .await;
    assert_eq!(
        out.iter()
            .filter(|s| s.starts_with("event: tool-call-end"))
            .count(),
        1
    );
    let end: Vec<_> = out
        .iter()
        .filter(|s| s.starts_with("event: message-end"))
        .collect();
    assert_eq!(end.len(), 1);
    assert!(end[0].contains("\"total_tokens\":5"));
    assert_eq!(stats.polls.load(std::sync::atomic::Ordering::Relaxed), 2);
}
