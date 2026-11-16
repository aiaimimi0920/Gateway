//! A public relay preserves the caller's real capabilities, not the default CLI's fixed surface.
use super::*;
use serde_json::json;
fn body() -> Value {
    json!({"model":"alias","messages":[{"role":"system","content":"caller policy"},
        {"role":"user","content":"hello"}],"temperature":0.25,"max_tokens":16})
}
#[test]
fn freebuff_public_relay_preserves_native_system_and_no_invented_cli_tools() {
    let step = RelayStep::build(body(), "fbm1.open", 400000, 65536).unwrap();
    assert_eq!(step.body["messages"], body()["messages"]);
    assert_eq!(step.body["model"], "fbm1.open");
    assert_eq!(step.body["temperature"], 0.25);
    assert_eq!(step.body["provider"], json!({"data_collection":"deny"}));
    assert!(step.body.get("tools").is_none());
    assert!(step.body.get("codebuff_metadata").is_none());
    assert_eq!(
        serde_json::from_slice::<Value>(&step.intent).unwrap(),
        step.body
    );
}
#[test]
fn freebuff_public_relay_keeps_tool_data_verbatim_and_does_not_repair_history() {
    let mut input = body();
    input["tools"] = json!([{"type":"function","function":{"name":"caller_tool","parameters":{"type":"object"}}}]);
    input["messages"].as_array_mut().unwrap().extend([
        json!({"role":"assistant","reasoning_details":[{"opaque":"native"}],"tool_calls":[{"id":"call-1","type":"function","function":{"name":"caller_tool","arguments":"{\"s\":\"&amp;\"}"},"extra_content":{"keep":true}}]}),
        json!({"role":"tool","tool_call_id":"call-1","content":"caller result"}),
    ]);
    let step = RelayStep::build(input.clone(), "fbm1.open", 400000, 65536).unwrap();
    assert_eq!(step.body["messages"], input["messages"]);
    assert_eq!(step.body["tools"], input["tools"]);
}
#[test]
fn freebuff_public_relay_rejects_authority_modality_and_unimplemented_fields() {
    for (name, value) in [
        ("codebuff_metadata", json!({"runId":"forged"})),
        ("provider", json!({"data_collection":"allow"})),
        ("stream", json!("true")),
        ("n", json!(2)),
        ("audio", json!({})),
        ("unknown", json!(true)),
    ] {
        let mut input = body();
        input[name] = value;
        assert!(
            RelayStep::build(input, "fbm1.open", 400000, 65536).is_err(),
            "{name}"
        );
    }
}
#[test]
fn freebuff_public_relay_checks_actual_context_before_any_purchase() {
    assert!(matches!(
        RelayStep::build(body(), "fbm1.open", 1, 65536),
        Err(BusinessError::ContextBudgetExceeded)
    ));
    assert!(RelayStep::build(body(), "fbm1.open", 400000, 16).is_err());
}
#[test]
fn freebuff_public_relay_rejects_bad_native_parameter_shapes_and_choices() {
    for (key, value) in [
        ("tool_choice", json!(7)),
        ("tool_choice", json!("required")),
        (
            "tool_choice",
            json!({"type":"function","function":{"name":"absent"}}),
        ),
        ("parallel_tool_calls", json!("yes")),
        ("stop", json!(["ok", 2])),
        ("stop", json!(["a", "b", "c", "d", "e"])),
        ("seed", json!(0.5)),
        ("reasoning_effort", json!("invented")),
        ("user", json!({"id":"caller"})),
        ("max_tokens", json!(0)),
        ("temperature", json!(3)),
    ] {
        let mut input = body();
        input[key] = value;
        assert!(
            RelayStep::build(input, "fbm1.open", 400000, 65536).is_err(),
            "{key}"
        );
    }
}
#[test]
fn freebuff_public_relay_rejects_nested_modalities_and_unimplemented_native_shapes() {
    for message in [
        json!({"role":"assistant","content":"text","audio":{"id":"hidden"}}),
        json!({"role":"user","content":[{"type":"text","text":"hello","input_audio":{}}]}),
        json!({"role":"user","content":[{"type":"input_audio","input_audio":{}}]}),
        json!({"role":"assistant","reasoning_details":"not-an-array"}),
        json!({"role":"assistant","tool_calls":[{"id":"call-1","type":"function","function":{"name":"caller_tool","arguments":"{}","audio":{}}}]}),
        json!({"role":"user","content":"text","tool_call_id":"wrong-role"}),
    ] {
        let mut input = body();
        input["messages"][1] = message;
        assert!(RelayStep::build(input, "fbm1.open", 400000, 65536).is_err());
    }
    let mut input = body();
    input["tools"] = json!([{"type":"function","function":{"name":"caller_tool","parameters":{},"strict":"yes"}}]);
    assert!(RelayStep::build(input, "fbm1.open", 400000, 65536).is_err());
}
#[test]
fn freebuff_public_relay_preserves_valid_native_parameters_and_tool_choice() {
    let mut input = body();
    input["tools"] = json!([{"type":"function","function":{"name":"caller_tool","description":"caller schema","parameters":{"type":"object"},"strict":true}}]);
    input["tool_choice"] = json!({"type":"function","function":{"name":"caller_tool"}});
    input["parallel_tool_calls"] = json!(false);
    input["stop"] = json!(["stop-one", "stop-two"]);
    input["seed"] = json!(-12);
    input["reasoning_effort"] = json!("high");
    input["user"] = json!("caller-label-is-not-authority");
    input["messages"][1]["content"] = json!([{"type":"text","text":"caller text"}]);
    let step = RelayStep::build(input.clone(), "fbm1.open", 400000, 65536).unwrap();
    for key in [
        "messages",
        "tools",
        "tool_choice",
        "parallel_tool_calls",
        "stop",
        "seed",
        "reasoning_effort",
        "user",
    ] {
        assert_eq!(step.body[key], input[key], "{key}");
    }
}
