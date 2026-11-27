use super::*;
use crate::protocol::cursor_remote::ingress::{self, Execution};

fn body(messages: Value) -> Value {
    json!({"model":"cursor-personal","messages":messages,
        "tools":[{"type":"function","function":{"name":"echo","parameters":{"type":"object"}}},
            {"type":"custom","custom":{"name":"edit","format":{"type":"text"}}}]})
}
fn parse(body: Value) -> Result<Execution, GatewayError> {
    ingress::parse(&crate::protocol::openai::normalize_chat_completions(body)?)
}
#[test]
fn cursor_remote_chat_raw_arguments_and_text_are_not_entity_decoded() {
    let arguments = " {\"text\":\"&quot;中文&amp;\"} ";
    let content = " \r\n结果<&amp;>\t";
    let req = body(json!([
        {"role":"user","content":"hello"},
        {"role":"assistant","content":null,"tool_calls":[{
            "id":"call_echo","type":"function","function":{"name":"echo","arguments":arguments}}]},
        {"role":"tool","tool_call_id":"call_echo","content":[
            {"type":"text","text":" \r\n"},{"type":"text","text":"结果<&amp;>\t"}]}
    ]));
    let Execution::Tools(request) = parse(req).unwrap() else {
        panic!("tool request");
    };
    assert_eq!(request.history[1]["arguments"], arguments);
    assert_eq!(request.history[2]["output"], content);
    assert_eq!(request.output.unwrap().text, content);
    assert!(request.prompt.is_empty());
    assert_eq!(request.history.len(), 3); // no synthetic empty assistant message
}

#[test]
fn cursor_remote_chat_custom_output_type_comes_from_exact_call() {
    let input = " \r\n*** 补丁\t&amp; ";
    let req = body(json!([
        {"role":"user","content":"hello"},
        {"role":"assistant","content":"prelude","tool_calls":[{
            "id":"call_edit","type":"custom","custom":{"name":"edit","input":input}}]},
        {"role":"tool","tool_call_id":"call_edit","content":""}
    ]));
    let Execution::Tools(request) = parse(req).unwrap() else {
        panic!("tool request");
    };
    assert_eq!(request.history[1]["content"], "prelude");
    assert_eq!(request.history[2]["input"], input);
    assert_eq!(request.history[3]["type"], "custom_tool_call_output");
    assert_eq!(request.output.unwrap().text, "");
}

#[test]
fn cursor_remote_chat_none_keeps_validated_declarations_out_of_worker() {
    let mut req = body(json!([{"role":"user","content":"hello"}]));
    req["tool_choice"] = json!("none");
    let Execution::Text(prompt) = parse(req.clone()).unwrap() else {
        panic!("text only");
    };
    assert!(prompt.prompt.contains("do not call tools"));
    req["tools"][0]["function"]["strict"] = json!(true);
    assert!(parse(req).is_err()); // none is not a schema-validation bypass
}

#[test]
fn cursor_remote_chat_rejects_lossy_fields_and_unmapped_controls() {
    let base = body(json!([{"role":"user","content":"hello"}]));
    for (key, value) in [
        ("max_tokens", json!(20)),
        ("max_completion_tokens", json!(20)),
        ("temperature", json!(0)),
        ("n", json!(2)),
        ("store", json!(true)),
        ("stream_options", json!({"include_usage":true})),
        ("tool_choice", json!("required")),
        ("reasoning_effort", json!("high")),
        ("response_format", json!({"type":"json_object"})),
        ("unknown", Value::Null),
    ] {
        let mut request = base.clone();
        request[key] = value;
        assert!(parse(request).is_err(), "{key}");
    }
    for message in [
        json!({"role":"user","content":"hello","name":"named"}),
        json!({"role":"user","content":[{"type":"image_url","image_url":{"url":"https://example.test"}}]}),
        json!({"role":"user","content":[{"type":"text","text":"hello","cache_control":{}}]}),
        json!({"role":"assistant","content":null,"function_call":{"name":"echo","arguments":"{}"}}),
        json!({"role":"tool","tool_call_id":"missing","content":"output"}),
    ] {
        assert!(parse(body(json!([message]))).is_err());
    }
    let mut request = base.clone();
    request["tools"][0]["function"]["type"] = json!("custom");
    assert!(parse(request).is_err());
    let mut request = base;
    request["tools"][1]["custom"]["format"] = json!({
        "type":"grammar","grammar":{"syntax":"regex","definition":".*"}});
    assert!(parse(request).is_err());
}

#[test]
fn cursor_remote_chat_no_call_replay_or_choice_change_during_live_history() {
    let mut req = body(json!([
        {"role":"user","content":"hello"},
        {"role":"assistant","content":"","tool_calls":[{
            "id":"call_echo","type":"function","function":{"name":"echo","arguments":"{}"}}]},
        {"role":"tool","tool_call_id":"call_echo","content":"ok"}
    ]));
    assert!(parse(req.clone()).is_ok());
    req["tool_choice"] = json!("none");
    assert!(parse(req.clone()).is_err());
    req.as_object_mut().unwrap().remove("tool_choice");
    req["messages"]
        .as_array_mut()
        .unwrap()
        .push(json!({"role":"user","content":"new task"}));
    assert!(parse(req).is_err());
}

#[test]
fn cursor_remote_chat_projection_does_not_relabel_pipeline_endpoint() {
    let req = crate::protocol::openai::normalize_chat_completions(
        json!({"model":"cursor-personal","messages":[{"role":"user","content":"hello"}]}),
    )
    .unwrap();
    assert_eq!(ingress::scope(&req).unwrap(), "chat.completions");
    assert!(matches!(ingress::parse(&req).unwrap(), Execution::Text(_)));
    assert_eq!(req.endpoint_kind, EndpointKind::ChatCompletions);
    assert!(req.raw_body.get("messages").is_some());
    assert!(req.raw_body.get("input").is_none());
}
