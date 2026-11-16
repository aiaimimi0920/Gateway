use super::*;
use crate::protocol::{canonical::ContentPart, gemini::api, openai};

fn call(id: &str, name: &str) -> Value {
    json!({"id":id,"type":"function","function":{"name":name,"arguments":"{\"city\":\"杭州\"}"}})
}
fn assistant(calls: Value) -> Value {
    json!({"role":"assistant","content":"before-tools","tool_calls":calls})
}
fn result(id: &str, ordinal: usize) -> Value {
    json!({"role":"tool","tool_call_id":id,"content":json!({"ordinal":ordinal}).to_string()})
}
fn history() -> Vec<Value> {
    vec![
        json!({"role":"user","content":"before"}),
        assistant(json!([call("a", "lookup"), call("b", "lookup")])),
        result("b", 2),
        result("a", 1),
        json!({"role":"user","content":"after"}),
    ]
}
fn request(messages: Vec<Value>) -> CanonicalRelayRequest {
    openai::normalize_chat_completions(json!({"model":"model","messages":messages})).unwrap()
}
fn project(req: &CanonicalRelayRequest) -> Result<Value, GatewayError> {
    normalize(
        EXPRESS,
        req,
        api::pack_request(req, "model", true)["contents"].take(),
        "model",
    )
}
fn rejects(messages: Vec<Value>) {
    let error = project(&request(messages)).unwrap_err();
    assert_eq!(error.http_status, Some(400));
    assert_eq!(
        error.code.as_deref(),
        Some("vertex_express_invalid_tool_history")
    );
}

#[test]
fn vertex_express_history_parallel_results_reorder_by_local_id_without_v1_ids() {
    for second_name in ["lookup", "other"] {
        let mut messages = history();
        messages[1]["tool_calls"][1] = call("b", second_name);
        let output = project(&request(messages)).unwrap();
        assert_eq!(output.as_array().unwrap().len(), 4);
        assert_eq!(output[0]["parts"][0]["text"], "before");
        assert_eq!(output[3]["parts"][0]["text"], "after");
        let functions: Vec<_> = output[1]["parts"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|p| p.get("functionCall"))
            .collect();
        assert_eq!(functions.len(), 2);
        assert!(functions.iter().all(|call| call.get("id").is_none()));
        let parts = output[2]["parts"].as_array().unwrap();
        assert_eq!(parts.len(), 2);
        for (i, name) in ["lookup", second_name].iter().enumerate() {
            assert_eq!(parts[i]["functionResponse"]["name"], *name);
            assert_eq!(parts[i]["functionResponse"]["response"]["ordinal"], i + 1);
            assert!(parts[i]["functionResponse"].get("id").is_none());
        }
    }
}

#[test]
fn vertex_express_history_rejects_missing_duplicate_unknown_or_cross_round_results() {
    for id in ["", "unknown", "a"] {
        let mut messages = history();
        messages[2]["tool_call_id"] = json!(id);
        rejects(messages);
    }
    let mut messages = history();
    messages.remove(3);
    rejects(messages);
    let mut messages = history();
    messages[2].as_object_mut().unwrap().remove("tool_call_id");
    rejects(messages);
    let mut messages = history();
    messages[2]["name"] = json!("conflicting");
    rejects(messages);
    let mut messages = history();
    messages.insert(3, json!({"role":"assistant","content":"interruption"}));
    rejects(messages);
    let mut messages = history();
    messages[1]["tool_calls"][1]["id"] = json!("a");
    rejects(messages);
    let mut messages = history();
    messages.extend([assistant(json!([call("c", "lookup")])), result("a", 3)]);
    rejects(messages);
}

#[test]
fn vertex_express_history_validates_original_arguments_and_blocks_raw_function_injection() {
    for args in [json!("{broken"), json!("[]"), json!("null")] {
        let mut messages = history();
        messages[1]["tool_calls"][0]["function"]["arguments"] = args;
        rejects(messages);
    }
    for part in [
        json!({"functionCall":{"name":"injected","args":{}}}),
        json!({"functionResponse":{"name":"injected","response":{}}}),
    ] {
        let mut req = request(history());
        req.messages[0]
            .content
            .push(ContentPart::Raw { value: part });
        assert_eq!(project(&req).unwrap_err().http_status, Some(400));
    }
    let mut messages = history();
    messages[1]["tool_calls"][0]["function"]
        .as_object_mut()
        .unwrap()
        .remove("arguments");
    assert!(
        project(&request(messages)).is_ok(),
        "omitted no-parameter arguments remain valid"
    );
}

#[test]
fn vertex_express_history_rounds_close_before_plain_messages_or_a_new_round() {
    let mut messages = history();
    messages.extend([
        assistant(json!([call("next", "other")])),
        result("next", 3),
        json!({"role":"assistant","content":"final"}),
    ]);
    let output = project(&request(messages)).unwrap();
    assert_eq!(output.as_array().unwrap().len(), 7);
    assert_eq!(
        output[5]["parts"][0]["functionResponse"]["response"]["ordinal"],
        3
    );
    assert_eq!(output[6]["parts"][0]["text"], "final");
}
