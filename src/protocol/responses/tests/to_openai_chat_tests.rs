use super::*;

#[tokio::test]
async fn translate_responses_text_stream_to_openai_chat_events() {
    let chunks = vec![
        Ok(Bytes::from(format_sse_event(
            Some("response.created"),
            &json!({
                "type": "response.created",
                "response": {
                    "id": "resp_test",
                    "model": "gpt-5.4",
                    "created_at": 1700000000
                }
            })
            .to_string(),
        ))),
        Ok(Bytes::from(format_sse_event(
            Some("response.output_text.delta"),
            &json!({
                "type": "response.output_text.delta",
                "delta": "Hel"
            })
            .to_string(),
        ))),
        Ok(Bytes::from(format_sse_event(
            Some("response.output_text.delta"),
            &json!({
                "type": "response.output_text.delta",
                "delta": "lo"
            })
            .to_string(),
        ))),
        Ok(Bytes::from(format_sse_event(
            Some("response.completed"),
            &json!({
                "type": "response.completed",
                "response": {
                    "id": "resp_test",
                    "model": "gpt-5.4",
                    "created_at": 1700000000,
                    "status": "completed",
                    "output": [{
                        "type": "message",
                        "role": "assistant",
                        "content": [{
                            "type": "output_text",
                            "text": "Hello"
                        }]
                    }],
                    "usage": {
                        "input_tokens": 5,
                        "output_tokens": 2,
                        "total_tokens": 7
                    }
                }
            })
            .to_string(),
        ))),
    ];

    let frames = collect_sse_frames(translate_responses_sse_to_openai_chat(
        make_bytes_stream(chunks),
        "gpt-5.4".to_string(),
    ))
    .await;

    assert!(
        frames
            .iter()
            .any(|frame| frame.data.contains(r#""content":"Hel""#)),
        "expected chat delta chunk: {frames:?}"
    );
    assert!(
        frames
            .iter()
            .any(|frame| frame.data.contains(r#""finish_reason":"stop""#)),
        "expected stop chunk: {frames:?}"
    );
    let stop = frames
        .iter()
        .find(|frame| frame.data.contains(r#""finish_reason":"stop""#))
        .map(|frame| serde_json::from_str::<Value>(&frame.data).unwrap())
        .unwrap();
    assert_eq!(stop["usage"]["total_tokens"], 7);
}

#[tokio::test]
async fn translate_responses_tool_stream_to_openai_chat_events() {
    let chunks = vec![
        Ok(Bytes::from(format_sse_event(
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
        ))),
        Ok(Bytes::from(format_sse_event(
            Some("response.function_call_arguments.delta"),
            &json!({
                "type": "response.function_call_arguments.delta",
                "output_index": 0,
                "item_id": "fc_call_1",
                "delta": "{\"city\":\"Hangzhou\"}"
            })
            .to_string(),
        ))),
        Ok(Bytes::from(format_sse_event(
            Some("response.completed"),
            &json!({
                "type": "response.completed",
                "response": {
                    "id": "resp_tool",
                    "model": "gpt-5.4",
                    "created_at": 1700000001,
                    "status": "tool_calls",
                    "output": [{
                        "type": "function_call",
                        "call_id": "call_1",
                        "name": "weather",
                        "arguments": "{\"city\":\"Hangzhou\"}"
                    }],
                    "usage": {
                        "input_tokens": 8,
                        "output_tokens": 3,
                        "total_tokens": 11
                    }
                }
            })
            .to_string(),
        ))),
    ];

    let frames = collect_sse_frames(translate_responses_sse_to_openai_chat(
        make_bytes_stream(chunks),
        "gpt-5.4".to_string(),
    ))
    .await;

    assert!(
        frames.iter().any(|frame| {
            frame.data.contains(r#""tool_calls":[{"#)
                && frame.data.contains(r#""id":"call_1""#)
                && frame.data.contains(r#""name":"weather""#)
        }),
        "expected tool call start chunk: {frames:?}"
    );
    assert!(
        frames.iter().any(|frame| frame
            .data
            .contains(r#""arguments":"{\"city\":\"Hangzhou\"}""#)),
        "expected tool call arguments chunk"
    );
    assert!(
        frames
            .iter()
            .any(|frame| frame.data.contains(r#""finish_reason":"tool_calls""#)),
        "expected tool_calls finish reason"
    );
}

#[tokio::test]
async fn translate_responses_completed_xml_tool_text_to_openai_chat_events() {
    let chunks = vec![
        Ok(Bytes::from(format_sse_event(
            Some("response.output_text.delta"),
            &json!({
                "type": "response.output_text.delta",
                "output_index": 0,
                "item_id": "msg_1",
                "delta": "<tool_calls>\n<tool_call>\n<tool_name>weather</tool_name>\n<parameters>{\"city\":\"Hangzhou\"}</parameters>\n</tool_call>\n</tool_calls>"
            })
            .to_string(),
        ))),
        Ok(Bytes::from(format_sse_event(
            Some("response.completed"),
            &json!({
                "type": "response.completed",
                "response": {
                    "id": "resp_xml_tool",
                    "model": "qwen3.5-flash",
                    "created_at": 1700000002_i64,
                    "status": "completed",
                    "output": [{
                        "id": "msg_1",
                        "type": "message",
                        "role": "assistant",
                        "status": "completed",
                        "content": [{
                            "type": "output_text",
                            "text": "<tool_calls>\n<tool_call>\n<tool_name>weather</tool_name>\n<parameters>{\"city\":\"Hangzhou\"}</parameters>\n</tool_call>\n</tool_calls>"
                        }]
                    }],
                    "usage": {
                        "input_tokens": 8,
                        "output_tokens": 4,
                        "total_tokens": 12
                    }
                }
            })
            .to_string(),
        ))),
    ];

    let frames = collect_sse_frames(translate_responses_sse_to_openai_chat(
        make_bytes_stream(chunks),
        "qwen3.5-flash".to_string(),
    ))
    .await;

    assert!(
        frames.iter().any(|frame| {
            frame.data.contains(r#""tool_calls":[{"#) && frame.data.contains(r#""name":"weather""#)
        }),
        "expected tool call start chunk from XML fallback: {frames:?}"
    );
    assert!(
        frames.iter().any(|frame| frame
            .data
            .contains(r#""arguments":"{\"city\":\"Hangzhou\"}""#)),
        "expected tool call arguments chunk from XML fallback: {frames:?}"
    );
    assert!(
        frames
            .iter()
            .any(|frame| frame.data.contains(r#""finish_reason":"tool_calls""#)),
        "expected tool_calls finish reason from XML fallback: {frames:?}"
    );
}
#[tokio::test]
async fn responses_to_openai_rejects_oversized_pending_frame_without_completing() {
    let valid = Bytes::from(format_sse_event(
        Some("response.output_text.delta"),
        &json!({
            "type": "response.output_text.delta",
            "delta": "Hi"
        })
        .to_string(),
    ));
    let max_frame_bytes = valid.len() + 8;
    let oversized = Bytes::from(vec![b'x'; max_frame_bytes + 1]);

    let (output, errors) =
        collect_stream_output(translate_responses_sse_to_openai_chat_with_limit(
            make_bytes_stream(vec![Ok(valid), Ok(oversized)]),
            "gpt-4o".to_string(),
            max_frame_bytes,
        ))
        .await;

    assert!(output.contains(r#""content":"Hi""#));
    assert!(!output.contains(r#""finish_reason":"stop""#));
    assert!(!output.contains("data: [DONE]"));
    assert_eq!(errors.len(), 1);
    assert!(errors[0].contains(&format!("{max_frame_bytes}-byte limit")));
}
