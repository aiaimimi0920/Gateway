//! Frame-local validation does not assert final tool JSON, finish markers or full-stream success.
use super::*;
fn frame(value: Value) -> Vec<u8> {
    format!("data: {value}\n\n").into_bytes()
}

#[test]
fn freebuff_catalog_observed_decoder_drains_one_frame_without_prefetching_later_cost() {
    let first = json!({"choices":[{"delta":{"content":"你好"}}],"usage":{"cost":0.125}});
    let second = json!({"choices":[{"finish_reason":"stop"}],"usage":{"cost":0.25}});
    let mut wire = frame(first.clone());
    wire.extend(frame(second.clone()));
    wire.extend_from_slice(b"data: [DONE]\n\n");
    let mut decoder = StreamDecoder::new();
    decoder.feed(wire.into()).unwrap();
    assert_eq!(decoder.next_frame().unwrap(), Some(first));
    assert_eq!(decoder.accounting().cost, Some(0.125));
    assert_eq!(decoder.next_frame().unwrap(), Some(second));
    assert_eq!(decoder.accounting().cost, Some(0.25));
    assert!(decoder.next_frame().unwrap().is_none());
    assert_eq!(
        decoder.finish().unwrap()["choices"][0]["message"]["content"],
        "你好"
    );
}

#[test]
fn freebuff_catalog_observed_decoder_fragmented_utf8_and_tool_arguments_remain_native() {
    let first = json!({"id":"answer","choices":[{"index":0,"delta":{"content":"你", "reasoning":"思考",
        "tool_calls":[{"index":0,"id":"call","type":"function","function":{"name":"lookup","arguments":"{\"x\":"}}]}}]});
    let second = json!({"choices":[{"index":0,"delta":{"content":"好", "tool_calls":[{"index":0,
        "function":{"arguments":"1}"},"extra_content":{"opaque":true}}]},"finish_reason":"tool_calls"}]});
    let mut wire = frame(first.clone());
    wire.extend(frame(second.clone()));
    let mut decoder = StreamDecoder::new();
    let mut events = Vec::new();
    for byte in wire {
        decoder.feed(Bytes::from(vec![byte])).unwrap();
        while let Some(event) = decoder.next_frame().unwrap() {
            events.push(event);
        }
    }
    assert_eq!(events, vec![first, second]);
    let body = decoder.finish().unwrap();
    assert_eq!(body["choices"][0]["message"]["reasoning"], "思考");
    assert_eq!(
        body["choices"][0]["message"]["tool_calls"][0]["function"]["arguments"],
        "{\"x\":1}"
    );
    assert_eq!(
        body["choices"][0]["message"]["tool_calls"][0]["extra_content"],
        json!({"opaque":true})
    );
}

#[test]
fn freebuff_catalog_observed_decoder_rejects_bad_frames_after_preserving_valid_reported_cost() {
    for body in [
        json!({"choices":[4],"usage":{"cost":0.125}}),
        json!({"choices":[],"usage":{"cost":0.125,"prompt_tokens":-1}}),
        json!({"choices":[{"delta":{"audio":{}}}],"usage":{"cost":0.125}}),
        json!({"choices":[],"error":{"message":"synthetic-secret"},"usage":{"cost":0.125}}),
    ] {
        let mut decoder = StreamDecoder::new();
        decoder.feed(frame(body).into()).unwrap();
        assert_eq!(
            decoder.next_frame().err(),
            Some(BusinessError::InvalidResponse)
        );
        assert_eq!(decoder.accounting().sdk_credits().unwrap(), 13);
    }
    let mut decoder = StreamDecoder::new();
    decoder
        .feed(b"event: error\ndata: {}\n\n".as_slice().into())
        .unwrap();
    assert_eq!(
        decoder.next_frame().err(),
        Some(BusinessError::InvalidResponse)
    );
}

#[test]
fn freebuff_catalog_observed_decoder_finish_does_not_flush_a_truncated_frame_or_open_tool() {
    for wire in [
        b"data: {}\n".to_vec(),
        frame(json!({"choices":[{"delta":{"tool_calls":[{"id":"call",
        "function":{"name":"lookup","arguments":"{"}}]},"finish_reason":"tool_calls"}]})),
    ] {
        let mut decoder = StreamDecoder::new();
        decoder.feed(wire.into()).unwrap();
        while decoder.next_frame().unwrap().is_some() {}
        assert_eq!(decoder.finish().err(), Some(BusinessError::InvalidResponse));
    }
}
