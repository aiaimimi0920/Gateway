//! Projection keeps native identities/fragments and checks all output before paid FINISH.
use super::*;
fn projection(kind: EndpointKind) -> Projection {
    Projection::new(kind, "public-model".into(), "public-id".into())
}
fn frame(delta: Value) -> Value {
    json!({"id":"native-id","model":"native-model","choices":[{"index":0,"delta":delta,"finish_reason":null}]})
}
fn completion(text: &str, tools: Value) -> Value {
    let mut value = json!({"id":"native-id","model":"native-model","choices":[{"index":0,"message":{"role":"assistant","content":text},"finish_reason":"stop"}],"usage":{"cost":0.125}});
    if !tools.is_null() {
        value["choices"][0]["message"]["tool_calls"] = tools;
        value["choices"][0]["finish_reason"] = json!("tool_calls");
    }
    value
}
fn events(bytes: &Bytes) -> Vec<Value> {
    std::str::from_utf8(bytes)
        .unwrap()
        .lines()
        .filter_map(|line| line.strip_prefix("data: "))
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}
#[test]
fn freebuff_projected_sse_late_tool_id_sparse_indices_and_lexical_arguments_share_native_association(
) {
    for kind in [EndpointKind::Responses, EndpointKind::Messages] {
        let mut p = projection(kind);
        let first = p.frame(frame(json!({"tool_calls":[{"index":7,"function":{"name":"lookup","arguments":"{ \"x\":"}}]}))).unwrap();
        assert!(!std::str::from_utf8(&first)
            .unwrap()
            .contains("input_json_delta"));
        let next = p.frame(frame(json!({"tool_calls":[{"index":7,"id":"native-call","function":{"arguments":"1 }"}}]}))).unwrap();
        let next = std::str::from_utf8(&next).unwrap();
        assert!(next.contains("native-call"));
        assert!(next.contains("lookup"));
        assert!(next.contains("1 }"));
        let third = p.frame(frame(json!({"tool_calls":[{"id":"second-call","function":{"name":"second","arguments":"{}"}}]}))).unwrap();
        assert!(std::str::from_utf8(&third).unwrap().contains("second-call"));
        // Index 0 is the free native slot, but the public order remains first-seen (7 then 0).
        let tail = p.finish(completion("", json!([
            {"id":"second-call","type":"function","function":{"name":"second","arguments":"{}"},"extra_content":{"identity":"second"}},
            {"id":"native-call","type":"function","function":{"name":"lookup","arguments":"{ \"x\":1 }"},"extra_content":{"identity":"first"}}
        ]))).unwrap();
        let events = events(&tail);
        if kind == EndpointKind::Responses {
            let response = &events.last().unwrap()["response"];
            assert_eq!(response["id"], "public-id");
            assert_eq!(response["output"][0]["call_id"], "native-call");
            assert_eq!(response["output"][0]["arguments"], "{ \"x\":1 }");
            assert_eq!(response["output"][1]["call_id"], "second-call");
            assert_eq!(
                response["gateway_freebuff"]["tool_calls"][0]["extra_content"]["identity"],
                "first"
            );
            assert_eq!(
                response["gateway_freebuff"]["tool_calls"][1]["extra_content"]["identity"],
                "second"
            );
            assert!(response.get("usage").is_none());
        } else {
            let delta = events
                .iter()
                .find(|e| e["type"] == "message_delta")
                .unwrap();
            assert_eq!(delta["stop_reason"], Value::Null);
            assert_eq!(delta["delta"]["stop_reason"], "tool_use");
            assert_eq!(
                delta["gateway_freebuff"]["tool_calls"][0]["extra_content"]["identity"],
                "first"
            );
            assert_eq!(
                delta["gateway_freebuff"]["tool_calls"][1]["extra_content"]["identity"],
                "second"
            );
            assert!(delta.get("usage").is_none());
            assert_eq!(events.last().unwrap()["type"], "message_stop");
        }
    }
}
#[test]
fn freebuff_projected_sse_native_metadata_and_unknown_usage_are_not_forged_thinking_or_counts() {
    for kind in [EndpointKind::Responses, EndpointKind::Messages] {
        let mut p = projection(kind);
        let native = frame(
            json!({"content":"中\n\"","reasoning_content":"opaque","reasoning_details":[{"signature":"not-anthropic"}]}),
        );
        let bytes = p.frame(native.clone()).unwrap();
        let ev = events(&bytes);
        assert_eq!(ev.last().unwrap()["frame"], native);
        assert!(ev.iter().all(|e| e["type"] != "thinking_delta"));
        let mut body = completion("中\n\"", Value::Null);
        body["choices"][0]["message"]["reasoning_content"] = json!("opaque");
        let tail = p.finish(body).unwrap();
        assert!(std::str::from_utf8(&tail).unwrap().contains("opaque"));
        assert!(!std::str::from_utf8(&tail).unwrap().contains("input_tokens"));
        if kind == EndpointKind::Responses {
            let all = ev.into_iter().chain(events(&tail));
            for (index, event) in all.enumerate() {
                assert_eq!(event["sequence_number"], index);
            }
        }
    }
}
#[test]
fn freebuff_projected_sse_cumulative_budget_counts_escaping_and_terminal_before_finish() {
    for kind in [EndpointKind::Responses, EndpointKind::Messages] {
        let body = completion("hello", Value::Null);
        let mut measure = projection(kind);
        let total = measure.finish(body.clone()).unwrap().len();
        let mut exact = projection(kind);
        exact.remaining = total;
        assert!(exact.finish(body.clone()).is_ok());
        assert_eq!(exact.remaining, 0);
        let mut short = projection(kind);
        short.remaining = total - 1;
        assert!(matches!(
            short.finish(body.clone()),
            Err(BusinessError::ResponseTooLarge)
        ));
        let frame = frame(json!({"content":"\n\"中"}));
        let mut p = projection(kind);
        p.frame(frame.clone()).unwrap();
        p.remaining = 1;
        assert!(matches!(
            p.frame(frame),
            Err(BusinessError::ResponseTooLarge)
        ));
    }
}
#[test]
fn freebuff_projected_sse_rejects_multiple_choices_and_unrepresentable_tool_input() {
    let mut p = projection(EndpointKind::Responses);
    let mut native = frame(json!({"content":"hello"}));
    native["choices"][0]["index"] = json!(1);
    assert!(p.frame(native).is_err());
    let array = completion(
        "",
        json!([{"id":"call","type":"function","function":{"name":"tool","arguments":"[]"}}]),
    );
    assert!(projection(EndpointKind::Messages)
        .finish(array.clone())
        .is_err());
    assert!(projection(EndpointKind::Responses).finish(array).is_ok());
}
