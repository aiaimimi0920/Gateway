//! Requests use only production-issued continuation witnesses; native history remains native.
use serde_json::{json, Value};

pub fn path(protocol: u8, stream: bool) -> &'static str {
    match protocol {
        0 => "/v1/chat/completions",
        1 => "/v1/messages",
        2 => "/v1/responses",
        _ if stream => "/model/fixture-model/converse-stream",
        _ => "/model/fixture-model/converse",
    }
}
pub fn first(protocol: u8) -> Value {
    match protocol {
        0 => {
            json!({"messages":[{"role":"user","content":"hello"}],"tools":[{"type":"function","function":{"name":"lookup","parameters":{"type":"object"}}}]})
        }
        1 => {
            json!({"max_tokens":32,"messages":[{"role":"user","content":"hello"}],"tools":[{"name":"lookup","input_schema":{"type":"object"}}]})
        }
        2 => {
            json!({"input":"hello","tools":[{"type":"function","name":"lookup","parameters":{"type":"object"}}]})
        }
        _ => {
            json!({"messages":[{"role":"user","content":[{"text":"hello"}]}],"toolConfig":{"tools":[{"toolSpec":{"name":"lookup","inputSchema":{"json":{"type":"object"}}}}]}})
        }
    }
}
pub fn request(protocol: u8, turns: &[Value], stream: bool) -> Value {
    let mut body = first(protocol);
    if protocol == 3 {
        body["system"] = json!([{"text":"policy-secret"}]);
        for turn in turns {
            body["messages"].as_array_mut().unwrap().extend([
                json!({"role":"assistant","content":turn["content"]}),
                json!({"role":"user","content":[{"toolResult":{"toolUseId":"call-a","content":[{"text":"done"}]}}]})]);
        }
        return body;
    }
    body["model"] = json!("fixture-model");
    body["stream"] = json!(stream);
    if protocol == 2 {
        body["input"] = json!([{"role":"user","content":"hello"}]);
    }
    let key = if protocol == 2 { "input" } else { "messages" };
    for _ in turns {
        let suffix = match protocol {
            0 => {
                json!([{"role":"assistant","content":"beforeafter","tool_calls":[{"id":"call-a","type":"function","function":{"name":"lookup","arguments":"{\"v\":\"&quot;\"}"}}]},{"role":"tool","tool_call_id":"call-a","content":"done"}])
            }
            1 => {
                json!([{"role":"assistant","content":[{"type":"text","text":"beforeafter"},{"type":"tool_use","id":"call-a","name":"lookup","input":{"v":"&quot;"}}]},{"role":"user","content":[{"type":"tool_result","tool_use_id":"call-a","content":"done"}]}])
            }
            _ => {
                json!([{"role":"assistant","content":[{"type":"output_text","text":"beforeafter"}]},{"type":"function_call","call_id":"call-a","name":"lookup","arguments":"{\"v\":\"&quot;\"}"},{"type":"function_call_output","call_id":"call-a","output":"done"}])
            }
        };
        body[key]
            .as_array_mut()
            .unwrap()
            .extend(suffix.as_array().unwrap().clone());
    }
    if !turns.is_empty() {
        body["gateway_bedrock_history"] = json!(turns
            .iter()
            .enumerate()
            .map(|(i, turn)| json!({"assistant_turn":i,"turn":turn}))
            .collect::<Vec<_>>());
    }
    body
}
