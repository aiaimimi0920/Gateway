//! Tool completion must carry actual identity, even when its fragments arrive separately.
use super::*;
fn accept(
    state: &mut Admission,
    delta: Value,
    finish: Value,
) -> Result<(Option<Bytes>, Option<Bytes>), ProtocolStreamError> {
    state.accept(SseFrame {
        event_name: None,
        data: json!({"choices":[{"index":0,"delta":delta,"finish_reason":finish}]}).to_string(),
    })
}
#[test]
fn incomplete_tool_identity_is_not_replaced_by_converter_defaults() {
    for reason in ["tool_calls", "stop", "length", "content_filter"] {
        for call in [
            json!({"index":0}),
            json!({"index":0,"id":"call","function":{"arguments":"{}"}}),
            json!({"index":0,"function":{"name":"lookup","arguments":"{}"}}),
            json!({"index":0,"id":"call","function":{"name":"lookup"}}),
        ] {
            assert!(accept(
                &mut Admission::default(),
                json!({"tool_calls":[call]}),
                json!(reason)
            )
            .is_err());
        }
    }
    assert!(accept(
        &mut Admission::default(),
        json!({"function_call":{"arguments":"{}"}}),
        json!("function_call")
    )
    .is_err());
}
#[test]
fn interleaved_tool_field_fragments_are_admitted_without_buffering_arguments() {
    let mut state = Admission::default();
    accept(
        &mut state,
        json!({"tool_calls":[{"index":0,"id":"call0"},{"index":1,"id":"call1"}]}),
        Value::Null,
    )
    .unwrap();
    accept(&mut state,json!({"tool_calls":[{"index":1,"function":{"name":"second"}},{"index":0,"function":{"name":"first"}}]}),Value::Null).unwrap();
    accept(&mut state,json!({"tool_calls":[{"index":0,"function":{"arguments":"{}"}},{"index":1,"function":{"arguments":"{}"}}]}),json!("tool_calls")).unwrap();
    state
        .accept(SseFrame {
            event_name: None,
            data: "[DONE]".into(),
        })
        .unwrap();
    assert!(state.complete());
}
