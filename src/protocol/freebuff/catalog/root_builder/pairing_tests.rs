//! Native-history repair vectors adapted from the fixed common util/messages tests.
use super::*;

fn call(id: &str) -> Value {
    json!({"id":id,"type":"function","function":{"name":"read_files","arguments":"{}"}})
}
fn assistant(calls: Vec<Value>) -> Value {
    json!({"role":"assistant","content":null,"tool_calls":calls})
}
fn result(id: &str) -> Value {
    json!({"role":"tool","tool_call_id":id,"content":"synthetic result"})
}
fn native(messages: Vec<Value>, digest: &str) -> Vec<Value> {
    let mut step = input();
    step.messages = messages;
    built(digest, step).unwrap()["messages"].as_array().unwrap()[1..].to_vec()
}

#[test]
fn freebuff_root_history_partial_parallel_calls_keep_answered_and_native_metadata() {
    let mut message = assistant(vec![call("complete"), call("interrupted")]);
    message["content"] = json!("working");
    message["reasoning_details"] = json!([{"signature":"synthetic","data":"keep"}]);
    message["host_extension"] = json!({"opaque":true});
    let mut expected = message.clone();
    expected["tool_calls"] = json!([call("complete")]);
    assert_eq!(
        native(vec![message, result("complete")], "unknown"),
        vec![expected, result("complete")]
    );
}

#[test]
fn freebuff_root_history_user_boundary_rejects_late_answer_but_preserves_results() {
    let user = json!({"role":"user","content":"continue"});
    assert_eq!(
        native(
            vec![assistant(vec![call("late")]), user.clone(), result("late")],
            "unknown"
        ),
        vec![user, result("late")]
    );
}

#[test]
fn freebuff_root_history_freestanding_and_duplicate_results_are_not_wire_sanitized() {
    let orphan = result("orphan");
    assert_eq!(
        native(vec![orphan.clone(), orphan.clone()], "unknown"),
        vec![orphan.clone(), orphan]
    );
}

#[test]
fn freebuff_root_history_reused_ids_need_a_result_after_each_call() {
    let user = json!({"role":"user","content":"start"});
    let first = assistant(vec![call("same"), call("same")]);
    assert_eq!(
        native(
            vec![
                user.clone(),
                first.clone(),
                result("same"),
                assistant(vec![call("same")])
            ],
            "unknown"
        ),
        vec![user, first, result("same")]
    );
}

#[test]
fn freebuff_root_history_native_provider_executed_is_not_a_trusted_capability() {
    let mut untrusted = call("unanswered");
    untrusted["providerExecuted"] = json!(true);
    let user = json!({"role":"user","content":"start"});
    assert_eq!(
        native(vec![user.clone(), assistant(vec![untrusted])], "unknown"),
        vec![user]
    );
}

#[test]
fn freebuff_root_history_generated_user_notes_do_not_interrupt_tool_pairing() {
    let user = json!({"role":"user","content":"start"});
    let message = assistant(vec![call("answered")]);
    let output = native(
        vec![user.clone(), message.clone(), result("answered")],
        "957291675c9f9132",
    );
    assert_eq!(output.len(), 4);
    assert_eq!(output[0], user);
    assert!(output[1]["content"]
        .as_str()
        .unwrap()
        .starts_with("<system>"));
    assert_eq!(output[2], message);
    assert_eq!(output[3], result("answered"));
}

#[test]
fn freebuff_root_history_unanswered_call_removal_preserves_native_nontext_payloads() {
    for payload in [
        json!({"content":[{"type":"text","text":"keep"}]}),
        json!({"reasoning_content":"keep"}),
        json!({"refusal":"keep"}),
        json!({"audio":{"id":"synthetic"}}),
    ] {
        let mut message = assistant(vec![call("lost")]);
        message
            .as_object_mut()
            .unwrap()
            .extend(payload.as_object().unwrap().clone());
        let mut expected = message.clone();
        expected.as_object_mut().unwrap().remove("tool_calls");
        assert_eq!(native(vec![message], "unknown"), vec![expected]);
    }
}

#[test]
fn freebuff_root_history_assistant_is_not_a_pairing_boundary() {
    let first = assistant(vec![call("complete")]);
    assert_eq!(
        native(
            vec![
                first.clone(),
                assistant(vec![call("lost")]),
                result("complete")
            ],
            "unknown"
        ),
        vec![first, result("complete")]
    );
}

#[test]
fn freebuff_root_history_malformed_pairing_fields_are_local_errors() {
    for message in [
        json!({"role":"assistant","tool_calls":"not-array"}),
        json!({"role":"assistant","tool_calls":[{}]}),
        json!({"role":"assistant","tool_calls":[{"id":""}]}),
        json!({"role":"assistant","content":{}}),
        json!({"role":"assistant","reasoning_content":[]}),
        json!({"role":"assistant","reasoning_details":"not-array"}),
        json!({"role":"tool","content":"missing id"}),
        json!({"role":"tool","tool_call_id":"","content":"empty id"}),
    ] {
        let mut step = input();
        step.messages.push(message);
        assert_eq!(
            built("unknown", step).err(),
            Some(BusinessError::InvalidInput)
        );
    }
}

#[test]
fn freebuff_root_history_raw_snapshot_is_counted_before_dropping_large_calls() {
    let mut step = input();
    let mut huge = call("lost");
    huge["function"]["arguments"] = json!("x".repeat(32768));
    step.messages.push(assistant(vec![huge]));
    assert_eq!(
        built_limit("unknown", step, 32768).err(),
        Some(BusinessError::RequestTooLarge)
    );
    let messages = vec![json!({"role":"user","content":"escaped\n\"界"})];
    let exact = serde_json::to_vec(&messages).unwrap().len();
    assert_eq!(
        super::super::history::prepare(messages.clone(), exact).unwrap(),
        messages
    );
    assert_eq!(
        super::super::history::prepare(messages.clone(), exact - 1).err(),
        Some(BusinessError::RequestTooLarge)
    );
    for limit in [0, 50 * 1024 * 1024 + 1] {
        assert_eq!(
            super::super::history::prepare(messages.clone(), limit).err(),
            Some(BusinessError::InvalidInput)
        );
    }
}
