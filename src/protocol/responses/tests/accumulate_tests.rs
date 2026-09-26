use super::*;

#[test]
fn accumulate_responses_sse_bytes_restores_tool_calls_from_item_events() {
    let sse = [
        format_sse_event(
            Some("response.output_item.added"),
            &json!({
                "type": "response.output_item.added",
                "output_index": 0,
                "item": {
                    "id": "fc_call_1",
                    "type": "function_call",
                    "status": "in_progress",
                    "call_id": "call_1",
                    "name": "weather",
                    "arguments": ""
                }
            })
            .to_string(),
        ),
        format_sse_event(
            Some("response.function_call_arguments.delta"),
            &json!({
                "type": "response.function_call_arguments.delta",
                "output_index": 0,
                "item_id": "fc_call_1",
                "delta": "{\"city\":\"Hangzhou\"}"
            })
            .to_string(),
        ),
        format_sse_event(
            Some("response.completed"),
            &json!({
                "type": "response.completed",
                "response": {
                    "id": "resp_1",
                    "model": "gpt-5.4",
                    "status": "completed",
                    "output": [],
                    "usage": {
                        "input_tokens": 8,
                        "output_tokens": 3,
                        "total_tokens": 11
                    }
                }
            })
            .to_string(),
        ),
    ]
    .join("");

    let response = accumulate_responses_sse_bytes(sse.as_bytes(), "gpt-5.4").unwrap();
    assert_eq!(response.tool_calls.len(), 1);
    assert_eq!(response.tool_calls[0].id.as_deref(), Some("call_1"));
    assert_eq!(response.tool_calls[0].name.as_deref(), Some("weather"));
    assert_eq!(
        response.tool_calls[0].arguments.as_deref(),
        Some("{\"city\":\"Hangzhou\"}")
    );
    assert_eq!(response.finish_reason.as_deref(), Some("tool_calls"));
}

#[tokio::test]
async fn accumulate_responses_sse_stream_accepts_payload_at_byte_limit() {
    let sse = format_sse_event(
        Some("response.output_text.delta"),
        &json!({
            "type": "response.output_text.delta",
            "delta": "bounded response"
        })
        .to_string(),
    );
    let split_at = sse.len() / 2;
    let chunks = vec![
        Ok(Bytes::copy_from_slice(&sse.as_bytes()[..split_at])),
        Ok(Bytes::copy_from_slice(&sse.as_bytes()[split_at..])),
    ];

    let response = accumulate_responses_sse_stream_with_limit(
        Box::pin(make_bytes_stream(chunks)),
        "gpt-5.4",
        sse.len(),
    )
    .await
    .unwrap();

    assert_eq!(response.text, "bounded response");
}

#[tokio::test]
async fn accumulate_responses_sse_stream_rejects_oversized_upstream() {
    let chunks = vec![
        Ok(Bytes::from_static(b"1234")),
        Ok(Bytes::from_static(b"5678")),
        Ok(Bytes::from_static(b"9")),
    ];

    let error = accumulate_responses_sse_stream_with_limit(
        Box::pin(make_bytes_stream(chunks)),
        "gpt-5.4",
        8,
    )
    .await
    .unwrap_err();

    assert_eq!(error.code.as_deref(), Some("responses_stream_too_large"));
}
