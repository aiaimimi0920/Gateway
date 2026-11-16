use super::*;
fn accept(
    state: &mut Admission,
    value: Value,
) -> Result<(Option<Bytes>, Option<Bytes>), ProtocolStreamError> {
    state.accept(SseFrame {
        event_name: None,
        data: value.to_string(),
    })
}
fn chunk(index: u64, finish: Value) -> Value {
    json!({"choices":[{"index":index,"delta":{"content":"OK"},"finish_reason":finish}]})
}
fn done(state: &mut Admission) -> Result<(Option<Bytes>, Option<Bytes>), ProtocolStreamError> {
    state.accept(SseFrame {
        event_name: None,
        data: "[DONE]".into(),
    })
}
#[test]
fn multi_choice_deltas_stay_live_and_each_choice_must_finish() {
    let mut state = Admission::default();
    let mixed = json!({"choices":[{"index":9,"delta":{"content":"A"},"finish_reason":"stop"},
        {"index":u64::MAX,"delta":{"content":"B"},"finish_reason":null}]});
    let (live, held) = accept(&mut state, mixed).unwrap();
    let live = String::from_utf8(live.unwrap().to_vec()).unwrap();
    assert!(live.contains('A') && live.contains('B'));
    assert!(held.is_some());
    assert!(!state.complete());
    accept(&mut state, chunk(u64::MAX, json!("length"))).unwrap();
    done(&mut state).unwrap();
    assert!(state.complete());
    assert!(done(&mut state).is_err());
}
#[test]
fn duplicate_finished_missing_or_unfinished_choices_are_rejected() {
    for value in [
        json!({}),
        json!({"choices":[]}),
        json!({"choices":[{"delta":{}}]}),
        json!({"choices":[{"index":0,"delta":{}},{"index":0,"delta":{}}]}),
        json!({"choices":[{"index":0,"delta":{},"finish_reason":"mystery"}]}),
        json!({"choices":[{"index":0,"delta":{},"finish_reason":"tool_calls"}]}),
        json!({"choices":[{"index":0,"delta":{"content":17}}]}),
    ] {
        assert!(accept(&mut Admission::default(), value).is_err());
    }
    let mut state = Admission::default();
    accept(&mut state, chunk(0, json!("stop"))).unwrap();
    assert!(accept(&mut state, chunk(0, Value::Null)).is_err());
    let mut state = Admission::default();
    accept(&mut state, chunk(0, Value::Null)).unwrap();
    assert!(done(&mut state).is_err());
    assert!(done(&mut Admission::default()).is_err());
}
#[test]
fn unique_choice_budget_does_not_allocate_by_maximum_index() {
    let mut state = Admission::default();
    for index in 0..128 {
        accept(&mut state, chunk(u64::MAX - index, json!("stop"))).unwrap();
    }
    assert!(accept(&mut state, chunk(0, json!("stop"))).is_err());
}
#[test]
fn aggregate_usage_is_final_and_validated_with_canonical_cache_priority() {
    let good = json!({"choices":[],"usage":{"prompt_tokens":8,"completion_tokens":2,"total_tokens":10,
        "prompt_tokens_details":{"cached_tokens":3},"input_tokens_details":{"cached_tokens":999}}});
    let mut invalids = Vec::new();
    for (key, value) in [
        ("total_tokens", json!(9)),
        ("prompt_tokens", json!(-1)),
        ("completion_tokens", json!(u64::MAX)),
        ("cache_read_input_tokens", json!(999)),
        ("input_tokens", json!(999)),
        ("output_tokens", json!(999)),
        ("cache_creation_input_tokens", json!(1)),
    ] {
        let mut bad = good.clone();
        bad["usage"][key] = value;
        invalids.push(bad);
    }
    let mut bad = good.clone();
    bad["usage"]["prompt_tokens_details"] = json!({});
    invalids.push(bad);
    for bad in invalids {
        let mut state = Admission::default();
        accept(&mut state, chunk(0, json!("stop"))).unwrap();
        assert!(accept(&mut state, bad).is_err());
    }
    let mut state = Admission::default();
    assert!(accept(&mut state, good.clone()).is_err());
    let mut state = Admission::default();
    accept(&mut state, chunk(0, json!("stop"))).unwrap();
    let (live, held) = accept(&mut state, good.clone()).unwrap();
    assert!(live.is_none() && held.is_some());
    assert!(accept(&mut state, good).is_err());
    done(&mut state).unwrap();
}
#[test]
fn sse_multiline_json_is_reserialized_without_field_injection() {
    let mut state = Admission::default();
    let value = json!({"choices":[{"index":0,"delta":{"content":"line\nevent: error"},"finish_reason":null}]}).to_string().replace("{\"choices\"", "{\n\"choices\"");
    let (live, _) = state
        .accept(SseFrame {
            event_name: Some("message".into()),
            data: value,
        })
        .unwrap();
    let live = String::from_utf8(live.unwrap().to_vec()).unwrap();
    assert_eq!(live.matches('\n').count(), 2);
    assert!(!live.contains("\nevent:"));
}
#[test]
fn empty_data_is_not_a_comment_before_or_after_done() {
    for completed in [false, true] {
        let mut state = Admission::default();
        if completed {
            accept(&mut state, chunk(0, json!("stop"))).unwrap();
            done(&mut state).unwrap();
        }
        assert!(state
            .accept(SseFrame {
                event_name: None,
                data: String::new()
            })
            .is_err());
    }
}
#[test]
fn redundant_responses_usage_aliases_cannot_select_another_metering_branch() {
    let usage = json!({"choices":[],"usage":{"prompt_tokens":8,"completion_tokens":2,"total_tokens":10,
        "input_tokens":8,"output_tokens":2,"prompt_tokens_details":{"cached_tokens":3},"input_tokens_details":{"cached_tokens":999}}});
    let mut state = Admission::default();
    accept(&mut state, chunk(0, json!("stop"))).unwrap();
    let (_, held) = accept(&mut state, usage.clone()).unwrap();
    let held = String::from_utf8(held.unwrap().to_vec()).unwrap();
    let normalized: Value =
        serde_json::from_str(held.strip_prefix("data: ").unwrap().trim()).unwrap();
    assert!(normalized["usage"].get("output_tokens").is_none());
    assert!(normalized["usage"].get("input_tokens").is_none());
    assert_eq!(
        normalized["usage"]["prompt_tokens_details"]["cached_tokens"],
        3
    );
    for key in ["input_tokens", "output_tokens"] {
        let mut bad = usage.clone();
        bad["usage"].as_object_mut().unwrap().remove(key);
        let mut state = Admission::default();
        accept(&mut state, chunk(0, json!("stop"))).unwrap();
        assert!(accept(&mut state, bad).is_err());
    }
}
#[test]
fn nested_responses_usage_cannot_poison_the_chat_usage_observer() {
    let mut value = chunk(0, Value::Null);
    value["response"] = json!({"usage":{"input_tokens":999,"output_tokens":999,
        "input_tokens_details":{"cached_tokens":999}}});
    assert!(accept(&mut Admission::default(), value).is_err());
}
