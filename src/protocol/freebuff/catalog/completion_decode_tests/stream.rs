//! Aggregate decoding uses the same strict incremental framing, never EOF-only success.
use super::*;

#[test]
fn freebuff_catalog_sse_same_function_parallel_ids_without_indices_remain_distinct() {
    for index in [None, Some(1), Some(1023)] {
        let mut decoder = StreamDecoder::new();
        let mut value = json!({"choices":[{"delta":{"tool_calls":[
        {"id":"one","function":{"name":"lookup","arguments":"{\"x\":1}"}},
        {"id":"two","function":{"name":"lookup","arguments":"{\"x\":2}"}}
    ]},"finish_reason":"tool_calls"}]});
        if let Some(index) = index {
            value["choices"][0]["delta"]["tool_calls"][0]["index"] = json!(index);
        }
        decoder.push(Bytes::from(frame(value))).unwrap();
        let body = decoder.finish().unwrap();
        let calls = body["choices"][0]["message"]["tool_calls"]
            .as_array()
            .unwrap();
        assert_eq!(calls.len(), 2);
        for (id, arguments) in [("one", "{\"x\":1}"), ("two", "{\"x\":2}")] {
            assert_eq!(
                calls.iter().find(|call| call["id"] == id).unwrap()["function"]["arguments"],
                arguments
            );
        }
    }
}

#[test]
fn freebuff_catalog_sse_total_budget_covers_single_and_cumulative_input() {
    use super::super::completion_decode::BODY_LIMIT;
    let mut decoder = StreamDecoder::new();
    assert_eq!(
        decoder.push(Bytes::from(vec![b'x'; BODY_LIMIT + 1])).err(),
        Some(BusinessError::ResponseTooLarge)
    );
    let mut comment = vec![b'x'; BODY_LIMIT / 16];
    comment[0] = b':';
    let end = comment.len();
    comment[end - 2..].copy_from_slice(b"\n\n");
    let comment = Bytes::from(comment);
    let mut decoder = StreamDecoder::new();
    for _ in 0..16 {
        decoder.push(comment.clone()).unwrap();
    }
    assert_eq!(
        decoder.push(Bytes::from_static(b"\n")).err(),
        Some(BusinessError::ResponseTooLarge)
    );
}
fn frame(value: Value) -> Vec<u8> {
    format!("data: {value}\n\n").into_bytes()
}

#[test]
fn freebuff_catalog_sse_fragmented_reasoning_tools_usage_and_last_cost_are_preserved() {
    let mut bytes = frame(
        json!({"id":"answer","model":"reported-model","choices":[{"index":0,
        "delta":{"role":"assistant","content":"你","reasoning":"thought","reasoning_details":[{"opaque":1}],
        "tool_calls":[{"index":0,"id":"tool","function":{"name":"lookup","arguments":"{\"x\":"}}]}}],
        "usage":{"cost":0.5}}),
    );
    bytes.extend(frame(json!({"choices":[{"index":0,"delta":{"content":"好",
        "tool_calls":[{"index":0,"function":{"arguments":"1}"},"extra_content":{"kept":true}}]},
        "finish_reason":"tool_calls"}]})));
    bytes.extend(frame(
        json!({"choices":[],"usage":{"prompt_tokens":3,"completion_tokens":4,
        "total_tokens":7,"cost":0.25,"cost_details":{"upstream_inference_cost":0.125}}}),
    ));
    bytes.extend_from_slice(b"data: [DONE]\n\n");
    let mut decoder = StreamDecoder::new();
    for chunk in bytes.chunks(1) {
        decoder.push(Bytes::copy_from_slice(chunk)).unwrap();
    }
    assert_eq!(decoder.accounting().sdk_credits().unwrap(), 40);
    let body = decoder.finish().unwrap();
    assert_eq!(body["choices"][0]["message"]["content"], "你好");
    assert_eq!(body["choices"][0]["message"]["reasoning"], "thought");
    assert_eq!(
        body["choices"][0]["message"]["reasoning_details"],
        json!([{"opaque":1}])
    );
    assert_eq!(
        body["choices"][0]["message"]["tool_calls"][0]["function"]["arguments"],
        "{\"x\":1}"
    );
    assert_eq!(body["usage"]["total_tokens"], 7);
}

#[test]
fn freebuff_catalog_sse_rejects_truncation_error_bad_utf8_and_data_after_terminal() {
    let complete =
        frame(json!({"choices":[{"index":0,"delta":{"content":"answer"},"finish_reason":"stop"}]}));
    let mut after_done = complete.clone();
    after_done.extend_from_slice(b"data: [DONE]\n\n");
    after_done.extend(complete.clone());
    for bytes in [
        b"data: {bad}\n\n".to_vec(),
        b"data: \xff\n\n".to_vec(),
        complete[..complete.len() - 1].to_vec(),
        frame(json!({"choices":[{"index":0,"delta":{"content":"partial"}}]})),
        frame(json!({"error":{"message":"synthetic-secret"},"choices":[]})),
        after_done,
    ] {
        let mut decoder = StreamDecoder::new();
        let result = decoder.push(Bytes::from(bytes));
        assert!(result.is_err() || decoder.finish().is_err());
    }
}

#[test]
fn freebuff_catalog_sse_requires_consistent_identity_choice_and_closed_tool_json() {
    for next in [
        json!({"id":"foreign","choices":[]}),
        json!({"choices":[{"index":0,"delta":{"content":"late"}}]}),
        json!({"choices":[{"index":0,"finish_reason":"length"}]}),
    ] {
        let mut decoder = StreamDecoder::new();
        decoder
            .push(Bytes::from(frame(json!({"id":"answer",
            "choices":[{"index":0,"delta":{"content":"answer"},"finish_reason":"stop"}]}))))
            .unwrap();
        assert!(decoder.push(Bytes::from(frame(next))).is_err());
    }
    let mut decoder = StreamDecoder::new();
    decoder.push(Bytes::from(frame(json!({"choices":[{"index":0,
        "delta":{"tool_calls":[{"index":0,"id":"tool","function":{"name":"lookup","arguments":"{"}}]},
        "finish_reason":"tool_calls"}]})))).unwrap();
    assert!(decoder.finish().is_err());
}
