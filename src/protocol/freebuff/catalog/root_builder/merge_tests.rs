//! Already-native payloads keep their representation; internal SDK part conversion is not replayed.
use super::*;

fn messages(history: Vec<Value>) -> Result<Value, BusinessError> {
    let mut step = input();
    step.messages = history;
    built("unknown", step)
}
#[test]
fn freebuff_root_history_consecutive_assistants_merge_reasoning_calls_and_later_metadata() {
    let call =
        json!({"id":"call","type":"function","function":{"name":"read_files","arguments":"{}"}});
    let body = messages(vec![
        json!({"role":"assistant","content":"first","reasoning_content":"think-", "reasoning":"alias-",
            "reasoning_details":[{"signature":"one"}],"host_extension":{"version":1},"first_only":true}),
        json!({"role":"assistant","content":"second","reasoning_content":"next","reasoning":"next",
            "reasoning_details":[{"signature":"two"}],"tool_calls":[call.clone()],"host_extension":{"version":2}}),
        json!({"role":"tool","tool_call_id":"call","content":"answer"}),
    ]).unwrap();
    assert_eq!(body["messages"].as_array().unwrap().len(), 3);
    assert_eq!(
        body["messages"][1],
        json!({"role":"assistant","content":"firstsecond",
        "reasoning_content":"think-next","reasoning":"alias-next",
        "reasoning_details":[{"signature":"one"},{"signature":"two"}],"tool_calls":[call],
        "host_extension":{"version":2},"first_only":true})
    );
}

#[test]
fn freebuff_root_history_native_content_arrays_append_without_flattening() {
    let first = json!({"type":"text","text":"first"});
    let second =
        json!({"type":"image_url","image_url":{"url":"https://example.invalid/synthetic"}});
    let body = messages(vec![
        json!({"role":"assistant","content":[first.clone()]}),
        json!({"role":"assistant","content":[second.clone()]}),
    ])
    .unwrap();
    assert_eq!(body["messages"][1]["content"], json!([first, second]));
    assert_eq!(body["messages"].as_array().unwrap().len(), 2);
}

#[test]
fn freebuff_root_history_empty_native_content_does_not_erase_previous_text() {
    let body = messages(vec![
        json!({"role":"assistant","content":"keep"}),
        json!({"role":"assistant","content":null,"reasoning_content":"think"}),
        json!({"role":"assistant","content":""}),
    ])
    .unwrap();
    assert_eq!(body["messages"].as_array().unwrap().len(), 2);
    assert_eq!(body["messages"][1]["content"], "keep");
    assert_eq!(body["messages"][1]["reasoning_content"], "think");
}

#[test]
fn freebuff_root_history_keeps_tool_and_user_boundaries_between_assistants() {
    let history = vec![
        json!({"role":"assistant","content":"one"}),
        json!({"role":"tool","tool_call_id":"standalone","content":"answer"}),
        json!({"role":"assistant","content":"two"}),
        json!({"role":"user","content":"next"}),
        json!({"role":"assistant","content":"three"}),
    ];
    assert_eq!(
        messages(history.clone()).unwrap()["messages"]
            .as_array()
            .unwrap()[1..],
        history
    );
}

#[test]
fn freebuff_root_history_opaque_handle_does_not_inherit_compiled_claude_prefill_restriction() {
    let mut step = input();
    step.messages = vec![json!({"role":"assistant","content":"unfinished"})];
    let body = built("957291675c9f9132", step).unwrap();
    assert_eq!(body["messages"].as_array().unwrap().len(), 2);
    assert_eq!(body["messages"][1]["content"], "unfinished");
}

#[test]
fn freebuff_root_history_mixed_nonempty_content_rejects_instead_of_inventing_parts() {
    for history in [
        vec![
            json!({"role":"assistant","content":"text"}),
            json!({"role":"assistant","content":[{"type":"text","text":"part"}]}),
        ],
        vec![
            json!({"role":"assistant","content":[{"type":"text","text":"part"}]}),
            json!({"role":"assistant","content":"text"}),
        ],
    ] {
        assert_eq!(messages(history).err(), Some(BusinessError::InvalidInput));
    }
    let body = messages(vec![
        json!({"role":"assistant","content":[]}),
        json!({"role":"assistant","content":"text"}),
    ])
    .unwrap();
    assert_eq!(body["messages"][1]["content"], "text");
}

#[test]
fn freebuff_root_history_long_assistant_run_keeps_append_order_without_prefix_clones() {
    let mut step = input();
    step.messages = (0..4096)
        .map(|_| json!({"role":"assistant","content":"x"}))
        .collect();
    // The raw fixture itself exceeds the small 32 KiB helper budget, before it is merged.
    let body = built_limit("unknown", step, 262144).unwrap();
    assert_eq!(body["messages"].as_array().unwrap().len(), 2);
    assert_eq!(body["messages"][1]["content"], "x".repeat(4096));
}
