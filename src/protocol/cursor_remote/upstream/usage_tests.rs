use super::*;
use crate::protocol::cursor_remote::upstream::{
    connect_stream::TextDecoderState,
    protobuf::{connect_frame, field_bytes, field_str, field_varint, fields},
    text_session::text_request,
    tool_stream::tool_request,
};

fn numbers(values: &[(u32, u64)]) -> Vec<u8> {
    values
        .iter()
        .flat_map(|(tag, value)| field_varint(*tag, *value).unwrap())
        .collect()
}

fn call(id: &str, input: u64, output: u64) -> Vec<u8> {
    [
        field_str(1, id).unwrap(),
        numbers(&[(2, input), (3, output), (4, 2), (5, 1), (6, 3)]),
    ]
    .concat()
}

fn event(tag: u32, bytes: &[u8]) -> Vec<u8> {
    connect_frame(
        &field_bytes(1, &field_bytes(tag, bytes).unwrap()).unwrap(),
        0,
    )
}

#[test]
fn cursor_remote_usage_prefers_terminal_over_deduplicated_call_totals() {
    let mut usage = Usage::default();
    usage.model_call(&call("a", 10, 5)).unwrap();
    usage.model_call(&call("a", 10, 5)).unwrap();
    usage.model_call(&call("b", 20, 6)).unwrap();
    usage
        .turn_end(&numbers(&[(1, 40), (2, 12), (3, 7), (4, 8), (5, 4)]))
        .unwrap();
    let result = usage.finish().unwrap().unwrap();
    assert_eq!(
        (
            result.prompt_tokens,
            result.completion_tokens,
            result.total_tokens
        ),
        (40, 12, 52)
    );
    assert_eq!(result.cache_read_input_tokens, Some(7));
    assert_eq!(result.cache_creation_input_tokens, Some(8));
    assert!(usage.model_call(&call("c", 1, 3)).is_err());
}

#[test]
fn cursor_remote_usage_absent_partial_zero_and_call_fallback_are_distinct() {
    let mut absent = Usage::default();
    absent.turn_end(&[]).unwrap();
    assert!(absent.finish().unwrap().is_none());
    let mut partial = Usage::default();
    partial.model_call(&call("a", 10, 5)).unwrap();
    partial.turn_end(&numbers(&[(1, 10)])).unwrap();
    assert!(partial.finish().unwrap().is_none());
    let mut zero = Usage::default();
    zero.turn_end(&numbers(&[(1, 0), (2, 0)])).unwrap();
    assert_eq!(zero.finish().unwrap().unwrap().total_tokens, 0);
    let mut fallback = Usage::default();
    fallback.model_call(&call("a", 10, 5)).unwrap();
    fallback.model_call(&call("a", 10, 5)).unwrap();
    fallback.model_call(&call("b", 20, 6)).unwrap();
    fallback.turn_end(&[]).unwrap();
    assert_eq!(fallback.finish().unwrap().unwrap().total_tokens, 41);
    fallback.split_phase();
    assert!(fallback.finish().unwrap().is_none());
}

#[test]
fn cursor_remote_usage_rejects_negative_overflow_conflicts_and_bounds_ids() {
    for wire in [
        numbers(&[(1, u64::MAX), (2, 1)]),
        numbers(&[(1, MAX_TOKENS), (2, 1)]),
        numbers(&[(1, 1), (1, 2), (2, 1)]),
        numbers(&[(1, 1), (2, 1), (5, 2)]),
        field_str(1, "not an int").unwrap(),
    ] {
        assert!(Usage::default().turn_end(&wire).is_err());
    }
    let mut usage = Usage::default();
    usage.model_call(&call("a", 10, 5)).unwrap();
    assert!(usage.model_call(&call("a", 10, 6)).is_err());
    assert!(usage.model_call(&call("", 10, 5)).is_err());
    assert!(usage.model_call(&call(&"a".repeat(257), 10, 5)).is_err());
    for i in 1..MAX_CALLS {
        usage.model_call(&call(&format!("a{i}"), 10, 5)).unwrap();
    }
    assert!(usage.model_call(&call("overflow", 10, 5)).is_err());
}

#[test]
fn cursor_remote_usage_wire_requests_and_success_terminal_are_bound() {
    for encoded in [
        text_request("default", "hello", 65536, &[]).unwrap(),
        tool_request("default", "hello", &[], 2, &[]).unwrap(),
    ] {
        let envelope = fields(&encoded[5..]).unwrap();
        let run = fields(envelope[0].bytes).unwrap();
        assert_eq!(run.iter().find(|f| f.tag == 37).unwrap().varint, 1);
    }
    let mut decoder = TextDecoderState::new_text().unwrap();
    decoder
        .push(&event(1, &field_str(1, "done").unwrap()))
        .unwrap();
    decoder.push(&event(31, &call("a", 10, 5))).unwrap();
    assert!(decoder.finish("default".into()).is_err());
    decoder.push(&event(14, &[])).unwrap();
    decoder.push(&connect_frame(b"{}", 2)).unwrap();
    assert_eq!(
        decoder
            .finish("default".into())
            .unwrap()
            .usage
            .unwrap()
            .total_tokens,
        15
    );
}
