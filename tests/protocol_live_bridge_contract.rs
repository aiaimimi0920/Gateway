//! Contracts exposed by real Responses-to-Chat provider calls.
use neuro_gateway::protocol::{
    openai::pack_openai,
    responses::{build_responses_success, normalize_responses},
};
use serde_json::json;

#[test]
fn responses_output_limit_is_translated_to_chat_limit() {
    let request = normalize_responses(json!({
        "model": "example", "input": "hello", "max_output_tokens": 128
    }))
    .unwrap();
    for (model, key) in [("example", "max_tokens"), ("o3", "max_completion_tokens")] {
        let body = pack_openai(&request, model, false);
        assert_eq!(body[key], 128);
        assert!(body.get("max_output_tokens").is_none());
    }
}

#[test]
fn responses_named_tool_choice_uses_chat_function_shape() {
    let request = normalize_responses(json!({
        "model": "example", "input": "hello",
        "tool_choice": {"type": "function", "name": "echo_nonce"}
    }))
    .unwrap();
    let body = pack_openai(&request, "example", false);
    assert_eq!(
        body["tool_choice"],
        json!({
            "type": "function", "function": {"name": "echo_nonce"}
        })
    );
}

#[test]
fn responses_success_uses_response_status_not_chat_finish_reason() {
    for (reason, expected) in [
        ("stop", "completed"),
        ("tool_calls", "completed"),
        ("length", "incomplete"),
        ("max_tokens", "incomplete"),
        ("content_filter", "incomplete"),
        ("unexpected_upstream_reason", "failed"),
    ] {
        let response =
            build_responses_success("resp_test", "example", "ok", None, &[], Some(reason));
        assert_eq!(response["status"], expected);
    }
}

#[tokio::test]
async fn truncated_responses_stream_emits_incomplete_terminal_event() {
    use bytes::Bytes;
    use futures::{stream, StreamExt};
    use neuro_gateway::protocol::responses::translate_openai_sse_to_responses;
    let upstream = stream::iter(vec![Ok(Bytes::from_static(
        b"data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"partial\"},\"finish_reason\":\"length\"}]}\n\ndata: [DONE]\n\n",
    ))]);
    let mut stream = Box::pin(translate_openai_sse_to_responses(
        Box::pin(upstream),
        "example".into(),
    ));
    let mut output = String::new();
    while let Some(chunk) = stream.next().await {
        output.push_str(std::str::from_utf8(&chunk.unwrap()).unwrap());
    }
    assert!(output.contains("event: response.incomplete\n"));
    assert!(!output.contains("event: response.completed\n"));
    assert!(output.contains("\"status\":\"incomplete\""));
}
