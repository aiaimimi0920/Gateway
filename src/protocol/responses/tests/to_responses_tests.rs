use super::*;

#[tokio::test]
async fn translate_openai_text_stream_to_responses_events() {
    let chunks = vec![
        Ok(Bytes::from_static(
            br#"data: {"id":"chatcmpl_1","model":"gpt-4o","choices":[{"index":0,"delta":{"role":"assistant","content":"Hel"},"finish_reason":null}]}

"#,
        )),
        Ok(Bytes::from_static(
            br#"data: {"id":"chatcmpl_1","model":"gpt-4o","choices":[{"index":0,"delta":{"content":"lo"},"finish_reason":null}]}

"#,
        )),
        Ok(Bytes::from_static(
            br#"data: {"id":"chatcmpl_1","model":"gpt-4o","choices":[{"index":0,"delta":{},"finish_reason":"stop"}],"usage":{"prompt_tokens":5,"completion_tokens":2,"total_tokens":7}}

"#,
        )),
        Ok(Bytes::from_static(b"data: [DONE]\n\n")),
    ];

    let frames = collect_sse_frames(translate_openai_sse_to_responses(
        make_bytes_stream(chunks),
        "gpt-4o".to_string(),
    ))
    .await;

    let event_names = frames
        .iter()
        .map(|frame| frame.event_name.clone().unwrap_or_default())
        .collect::<Vec<_>>();
    assert_eq!(
        event_names.first().map(String::as_str),
        Some("response.created")
    );
    assert!(event_names.contains(&"response.output_text.delta".to_string()));
    assert_eq!(
        event_names.last().map(String::as_str),
        Some("response.completed")
    );

    let completed = frames
        .iter()
        .find(|frame| frame.event_name.as_deref() == Some("response.completed"))
        .map(|frame| serde_json::from_str::<Value>(&frame.data).unwrap())
        .unwrap();
    assert_eq!(completed["response"]["status"], "completed");
    assert_eq!(
        completed["response"]["output"][0]["content"][0]["text"],
        "Hello"
    );
    assert_eq!(completed["response"]["usage"]["input_tokens"], 5);
    assert_eq!(completed["response"]["usage"]["output_tokens"], 2);
}

#[tokio::test]
async fn translate_openai_tool_stream_to_responses_events() {
    let chunks = vec![
        Ok(Bytes::from_static(
            br#"data: {"id":"chatcmpl_1","model":"gpt-4o","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"id":"call_1","type":"function","function":{"name":"weather","arguments":""}}]},"finish_reason":null}]}

"#,
        )),
        Ok(Bytes::from_static(
            br#"data: {"id":"chatcmpl_1","model":"gpt-4o","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":"{\"city\":\"Hang"}}]},"finish_reason":null}]}

"#,
        )),
        Ok(Bytes::from_static(
            br#"data: {"id":"chatcmpl_1","model":"gpt-4o","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":"zhou\"}"}}]},"finish_reason":null}]}

"#,
        )),
        Ok(Bytes::from_static(
            br#"data: {"id":"chatcmpl_1","model":"gpt-4o","choices":[{"index":0,"delta":{},"finish_reason":"tool_calls"}],"usage":{"prompt_tokens":8,"completion_tokens":4,"total_tokens":12}}

"#,
        )),
        Ok(Bytes::from_static(b"data: [DONE]\n\n")),
    ];

    let frames = collect_sse_frames(translate_openai_sse_to_responses(
        make_bytes_stream(chunks),
        "gpt-4o".to_string(),
    ))
    .await;

    assert!(
        frames.iter().any(|frame| {
            frame.event_name.as_deref() == Some("response.function_call_arguments.delta")
                && frame.data.contains(r#""delta":"{\"city\":\"Hang""#)
        }),
        "expected function call argument delta event"
    );
    assert!(
        frames.iter().any(|frame| {
            frame.event_name.as_deref() == Some("response.function_call_arguments.done")
                && frame
                    .data
                    .contains(r#""arguments":"{\"city\":\"Hangzhou\"}""#)
        }),
        "expected function call argument done event"
    );

    let completed = frames
        .iter()
        .find(|frame| frame.event_name.as_deref() == Some("response.completed"))
        .map(|frame| serde_json::from_str::<Value>(&frame.data).unwrap())
        .unwrap();
    assert_eq!(completed["response"]["status"], "completed");
    assert_eq!(completed["response"]["output"][0]["type"], "function_call");
    assert_eq!(completed["response"]["output"][0]["id"], "fc_call_1");
    assert_eq!(completed["response"]["output"][0]["call_id"], "call_1");
    assert_eq!(
        completed["response"]["output"][0]["arguments"],
        "{\"city\":\"Hangzhou\"}"
    );
}
#[tokio::test]
async fn openai_to_responses_rejects_oversized_pending_frame_without_completing() {
    let valid = Bytes::from_static(
        br#"data: {"id":"chatcmpl_1","model":"gpt-4o","choices":[{"index":0,"delta":{"content":"Hi"},"finish_reason":null}]}

"#,
    );
    let max_frame_bytes = valid.len() + 8;
    let oversized = Bytes::from(vec![b'x'; max_frame_bytes + 1]);

    let (output, errors) = collect_stream_output(translate_openai_sse_to_responses_with_limit(
        make_bytes_stream(vec![Ok(valid), Ok(oversized)]),
        "gpt-4o".to_string(),
        max_frame_bytes,
    ))
    .await;

    assert!(output.contains("response.output_text.delta"));
    assert!(!output.contains("response.completed"));
    assert!(!output.contains("data: [DONE]"));
    assert_eq!(errors.len(), 1);
    assert!(errors[0].contains(&format!("{max_frame_bytes}-byte limit")));
}
#[tokio::test]
async fn translate_native_responses_stream_passthroughs() {
    let created = br#"event: response.created
data: {"type":"response.created","response":{"id":"resp_1","model":"gpt-4o","created_at":1}}

"#;
    let completed = br#"event: response.completed
data: {"type":"response.completed","response":{"id":"resp_1","usage":{"input_tokens":1,"output_tokens":2,"total_tokens":3}}}

"#;
    let done = b"data: [DONE]\n\n";
    let chunks = vec![
        Ok(Bytes::from_static(created)),
        Ok(Bytes::from_static(completed)),
        Ok(Bytes::from_static(done)),
    ];

    let translated =
        translate_openai_sse_to_responses(make_bytes_stream(chunks), "gpt-4o".to_string())
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .map(|chunk| String::from_utf8_lossy(&chunk.unwrap()).to_string())
            .collect::<String>();

    let original = [
        String::from_utf8_lossy(created).to_string(),
        String::from_utf8_lossy(completed).to_string(),
        String::from_utf8_lossy(done).to_string(),
    ]
    .join("");

    assert_eq!(translated, original);
}

#[tokio::test]
async fn translate_native_responses_stops_at_done_with_trailing_same_chunk_data() {
    let input = Bytes::from_static(
        b"event: response.created\n\
data: {\"type\":\"response.created\",\"response\":{\"id\":\"resp_1\",\"model\":\"gpt-4o\",\"created_at\":1}}\n\
\n\
data: [DONE]\n\
\n\
event: response.completed\n\
data: {\"type\":\"response.completed\",\"unexpected\":true}\n\
\n",
    );

    let (translated, errors) = collect_stream_output(translate_openai_sse_to_responses(
        make_bytes_stream(vec![Ok(input)]),
        "gpt-4o".to_string(),
    ))
    .await;

    assert!(errors.is_empty());
    assert!(translated.contains("response.created"));
    assert!(translated.ends_with("data: [DONE]\n\n"));
    assert!(!translated.contains("unexpected"));
    assert!(!translated.contains("response.completed"));
}

#[tokio::test]
async fn translate_native_responses_error_passthroughs() {
    let error = br#"event: error
data: {"type":"error","sequence_number":1,"code":"bad_request","message":"boom"}

"#;
    let done = b"data: [DONE]\n\n";
    let chunks = vec![Ok(Bytes::from_static(error)), Ok(Bytes::from_static(done))];

    let translated =
        translate_openai_sse_to_responses(make_bytes_stream(chunks), "gpt-4o".to_string())
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .map(|chunk| String::from_utf8_lossy(&chunk.unwrap()).to_string())
            .collect::<String>();

    let original = [
        String::from_utf8_lossy(error).to_string(),
        String::from_utf8_lossy(done).to_string(),
    ]
    .join("");

    assert_eq!(translated, original);
}

#[tokio::test]
async fn translate_openai_tool_stream_waits_for_real_name_before_added_event() {
    let chunks = vec![
        Ok(Bytes::from_static(
            br#"data: {"id":"chatcmpl_1","model":"gpt-4o","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"id":"call_1","type":"function","function":{"arguments":"{\"city\":\"Hang"}}]},"finish_reason":null}]}

"#,
        )),
        Ok(Bytes::from_static(
            br#"data: {"id":"chatcmpl_1","model":"gpt-4o","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"name":"weather","arguments":"zhou\"}"}}]},"finish_reason":null}]}

"#,
        )),
        Ok(Bytes::from_static(
            br#"data: {"id":"chatcmpl_1","model":"gpt-4o","choices":[{"index":0,"delta":{},"finish_reason":"tool_calls"}],"usage":{"prompt_tokens":8,"completion_tokens":4,"total_tokens":12}}

"#,
        )),
        Ok(Bytes::from_static(b"data: [DONE]\n\n")),
    ];

    let frames = collect_sse_frames(translate_openai_sse_to_responses(
        make_bytes_stream(chunks),
        "gpt-4o".to_string(),
    ))
    .await;

    let added = frames
        .iter()
        .find(|frame| frame.event_name.as_deref() == Some("response.output_item.added"))
        .map(|frame| serde_json::from_str::<Value>(&frame.data).unwrap())
        .unwrap();
    assert_eq!(added["item"]["type"], "function_call");
    assert_eq!(added["item"]["name"], "weather");

    let completed = frames
        .iter()
        .find(|frame| frame.event_name.as_deref() == Some("response.completed"))
        .map(|frame| serde_json::from_str::<Value>(&frame.data).unwrap())
        .unwrap();
    assert_eq!(completed["response"]["output"][0]["id"], "fc_call_1");
    assert_eq!(completed["response"]["output"][0]["call_id"], "call_1");
    assert_eq!(completed["response"]["output"][0]["name"], "weather");
    assert_eq!(
        completed["response"]["output"][0]["arguments"],
        "{\"city\":\"Hangzhou\"}"
    );
}

#[tokio::test]
async fn translate_anthropic_like_stream_to_responses_events() {
    let chunks = vec![
        Ok(Bytes::from_static(
            br#"event: message_start
data: {"type":"message_start","message":{"id":"msg_1","usage":{"input_tokens":13,"output_tokens":0}}}

"#,
        )),
        Ok(Bytes::from_static(
            br#"event: content_block_start
data: {"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}

"#,
        )),
        Ok(Bytes::from_static(
            br#"event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hi"}}

"#,
        )),
        Ok(Bytes::from_static(
            br#"event: content_block_stop
data: {"type":"content_block_stop","index":0}

"#,
        )),
        Ok(Bytes::from_static(
            br#"event: message_delta
data: {"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":9,"cache_read_input_tokens":4}}

"#,
        )),
        Ok(Bytes::from_static(
            br#"event: message_stop
data: {"type":"message_stop"}

"#,
        )),
    ];

    let openai_stream = accio::translate_anthropic_like_stream_to_openai(
        make_bytes_stream(chunks),
        "claude-sonnet-4-6".to_string(),
    );
    let openai_chunks = openai_stream
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .map(|chunk| String::from_utf8_lossy(&chunk.unwrap()).to_string())
        .collect::<String>();
    let frames = collect_sse_frames(translate_openai_sse_to_responses(
        make_bytes_stream(vec![Ok(Bytes::from(openai_chunks.clone()))]),
        "claude-sonnet-4-6".to_string(),
    ))
    .await;

    let completed = frames
        .iter()
        .find(|frame| frame.event_name.as_deref() == Some("response.completed"))
        .map(|frame| serde_json::from_str::<Value>(&frame.data).unwrap())
        .unwrap();
    assert_eq!(completed["response"]["status"], "completed");
    assert!(completed["response"]["output"][0]["id"].as_str().is_some());
    assert_eq!(completed["response"]["output"][0]["status"], "completed");
    assert_eq!(
        completed["response"]["output"][0]["content"][0]["text"],
        "Hi"
    );
    assert_eq!(completed["response"]["usage"]["input_tokens"], 13);
    assert_eq!(completed["response"]["usage"]["output_tokens"], 9);
    assert_eq!(
        completed["response"]["usage"]["input_tokens_details"]["cached_tokens"],
        4
    );
}
