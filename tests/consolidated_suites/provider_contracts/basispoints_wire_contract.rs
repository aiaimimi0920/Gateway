//! Offline product-channel contracts: client tools must never become executable Office tools.
use neuro_gateway::protocol::chatgpt::basispoints::{request, response};
use serde_json::{json, Value};

fn source() -> Value {
    json!({"input":"hello", "tools":[{"type":"function", "name":"exec", "parameters":{
        "type":"object", "required":["cmd"], "properties":{"cmd":{"type":"string"}}, "additionalProperties":false}}]})
}
fn reply(tool: &str, args: Value) -> Value {
    json!({"id":"r1", "status":"completed", "output":[{"id":"fc1", "type":"function_call", "call_id":"c1",
        "name":"run_officejs", "arguments":json!({"code":json!({"tool":tool,"args":args}).to_string()}).to_string()}]})
}
fn decode(body: &Value, source: &Value) -> Result<Value, neuro_gateway::error::GatewayError> {
    response::decode(&serde_json::to_vec(body).unwrap(), false, source)
}

#[test]
fn request_preserves_constraints_and_stable_uuid_conversation() {
    let mut input = source();
    input["context_management"] = json!([]);
    input["text"] = json!({"format":{"type":"json_object"}});
    input["max_output_tokens"] = json!(19);
    let first = request::prepare(&input, "gpt-fixture", true).unwrap();
    assert_eq!(first["store"], false);
    assert_eq!(first["model_selection"], "explicit");
    assert_eq!(first["max_output_tokens"], 19);
    assert_eq!(first["text"], input["text"]);
    assert!(first.get("tools").is_none());
    assert!(first.get("context_management").is_none());
    assert_eq!(first["metadata"]["agent_iteration"], "1");
    assert_eq!(
        uuid::Uuid::parse_str(first["metadata"]["task_id"].as_str().unwrap())
            .unwrap()
            .get_version_num(),
        5
    );
    input["input"] = json!([{"role":"user","content":[{"type":"input_text","text":"hello"}]},
        {"role":"assistant","content":[{"type":"output_text","text":"answer"}]},
        {"role":"user","content":"next"}]);
    let next = request::prepare(&input, "gpt-fixture", false).unwrap();
    assert_eq!(first["metadata"]["task_id"], next["metadata"]["task_id"]);
    assert_ne!(first["metadata"]["turn_id"], next["metadata"]["turn_id"]);
}

#[test]
fn function_history_roundtrip_is_stateless_and_retains_call_identity() {
    let mut input = source();
    let output = decode(&reply("exec", json!({"cmd":"pwd"})), &input).unwrap();
    assert_eq!(output["output"][0]["name"], "exec");
    input["input"] = json!([{"role":"user","content":"hello"}, output["output"][0],
        {"type":"function_call_output","call_id":"c1","output":"result"},
        {"type":"reasoning","summary":[],"encrypted_content":"opaque"}]);
    let packed = request::prepare(&input, "fixture", true).unwrap();
    let items = packed["input"].as_array().unwrap();
    assert!(items
        .iter()
        .any(|i| i["name"] == "run_officejs" && i["call_id"] == "c1"));
    assert_eq!(packed["metadata"]["agent_iteration"], "2");
    input["input"]
        .as_array_mut()
        .unwrap()
        .push(json!({"type":"function_call_output","call_id":"c1","output":"again"}));
    assert!(request::prepare(&input, "fixture", true).is_err());
}

#[test]
fn custom_namespace_and_none_choice_do_not_corrupt_history() {
    let mut input = json!({"input":"hello", "tools":[{"type":"namespace","name":"shell","tools":[{"type":"custom","name":"patch"}]}]});
    let output = decode(&reply("shell.patch", json!("raw patch\ntext")), &input).unwrap();
    assert_eq!(output["output"][0]["type"], "custom_tool_call");
    assert_eq!(output["output"][0]["namespace"], "shell");
    assert_eq!(output["output"][0]["input"], "raw patch\ntext");
    input["tool_choice"] = json!("none");
    assert!(decode(&reply("shell.patch", json!("raw")), &input).is_err());
    input["input"] = json!([{"role":"user","content":"hello"},output["output"][0],
        {"type":"custom_tool_call_output","call_id":"c1","output":"ok"}]);
    assert!(request::prepare(&input, "fixture", true).is_ok());
    input["tool_choice"] = json!({"type":"function","name":"patch","namespace":"shell"});
    assert!(request::prepare(&input, "fixture", true).is_err());
}

#[test]
fn invalid_tool_arguments_native_tools_and_parallel_violations_are_rejected() {
    let input = source();
    for args in [
        json!({}),
        json!({"cmd":7}),
        json!({"cmd":"pwd","extra":true}),
        json!("not an object"),
    ] {
        assert!(
            decode(&reply("exec", args.clone()), &input).is_err(),
            "{args}"
        );
    }
    assert!(decode(&reply("unknown", json!({})), &input).is_err());
    let mut native = reply("exec", json!({"cmd":"pwd"}));
    native["output"][0]["type"] = json!("web_search_call");
    assert!(decode(&native, &input).is_err());
    let mut required = source();
    required["tool_choice"] = json!("required");
    assert!(decode(&json!({"status":"completed","output":[]}), &required).is_err());
    let mut two = reply("exec", json!({"cmd":"pwd"}));
    let mut second = two["output"][0].clone();
    second["call_id"] = json!("c2");
    two["output"].as_array_mut().unwrap().push(second);
    let mut single = source();
    single["parallel_tool_calls"] = json!(false);
    assert!(decode(&two, &single).is_err());
}

#[test]
fn only_a_completed_response_is_success_and_replay_has_standard_lifecycle() {
    let body = reply("exec", json!({"cmd":"pwd"}));
    let sse = format!(
        "data: {}\n\n",
        json!({"type":"response.completed","response":body})
    );
    let converted = response::decode(sse.as_bytes(), true, &source()).unwrap();
    for invalid in [
        "data: [DONE]\n\n".to_owned(),
        sse.repeat(2),
        format!("{sse}data: {{\"type\":\"response.failed\"}}\n\n"),
    ] {
        assert!(response::decode(invalid.as_bytes(), true, &source()).is_err());
    }
    let replay = String::from_utf8(response::replay(&converted)).unwrap();
    assert!(replay.contains("response.function_call_arguments.done"));
    assert!(!replay.contains("run_officejs"));
    let text = json!({"id":"r", "status":"completed", "output":[{"type":"message","id":"m", "role":"assistant",
        "content":[{"type":"output_text","text":"answer","annotations":[]}]}]});
    let replay = String::from_utf8(response::replay(&text)).unwrap();
    for event in [
        "response.content_part.added",
        "response.output_text.delta",
        "response.output_text.done",
        "response.content_part.done",
    ] {
        assert!(replay.contains(event), "{event}");
    }
}

#[test]
fn unsupported_continuations_and_images_fail_explicitly() {
    for input in [
        json!({"input":"x","previous_response_id":"r"}),
        json!({"input":"x","background":true}),
        json!({"input":[{"role":"user","content":[{"type":"input_image","image_url":"https://fixture.invalid/x"}]}]}),
        json!({"input":"x","tool_choice":"surprise"}),
        json!({"input":[]}),
    ] {
        assert!(request::prepare(&input, "fixture", true).is_err());
    }
}
