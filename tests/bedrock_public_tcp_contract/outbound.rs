//! Inspect real public bytes, including standard text/tool deltas, not just extension metadata.
use super::{fixture, native_wire};
use bytes::Bytes;
use serde_json::{json, Value};

pub fn values(bytes: &[u8]) -> Vec<Value> {
    std::str::from_utf8(bytes)
        .unwrap()
        .split("\n\n")
        .filter_map(|frame| frame.lines().find_map(|line| line.strip_prefix("data: ")))
        .filter(|data| *data != "[DONE]")
        .map(|data| serde_json::from_str(data).unwrap())
        .collect()
}
pub fn native(mut bytes: &[u8]) -> Vec<(String, Value)> {
    let mut events = Vec::new();
    while !bytes.is_empty() {
        assert!(bytes.len() >= 16);
        let size = u32::from_be_bytes(bytes[..4].try_into().unwrap()) as usize;
        assert!(size >= 16 && size <= bytes.len());
        events.push(native_wire::decode(&Bytes::copy_from_slice(&bytes[..size])));
        bytes = &bytes[size..];
    }
    events
}
pub fn public(protocol: u8, bytes: &[u8], stream: bool) -> Value {
    assert!(!String::from_utf8_lossy(bytes).contains("policy-secret"));
    if !stream {
        let value: Value = serde_json::from_slice(bytes).unwrap();
        let decoded = (fixture::public_body(protocol).1)(&value).unwrap();
        assert_eq!(decoded.text, "beforeafter");
        assert_eq!(decoded.finish_reason.as_deref(), Some("tool_calls"));
        assert_eq!(decoded.usage.unwrap().total_tokens, 7);
        assert_eq!(decoded.tool_calls.len(), 1);
        let tool = &decoded.tool_calls[0];
        assert_eq!(tool.id.as_deref(), Some("call-a"));
        assert_eq!(tool.name.as_deref(), Some("lookup"));
        assert_eq!(
            serde_json::from_str::<Value>(tool.arguments.as_deref().unwrap()).unwrap(),
            json!({"v":"&quot;"})
        );
        return value["gateway_bedrock_turn"].clone();
    }
    let frames = values(bytes);
    assert_eq!(
        frames
            .iter()
            .filter(|v| v.get("gateway_bedrock_event").is_some())
            .count(),
        fixture::sequence().len()
    );
    let mut text = String::new();
    let mut arguments = String::new();
    let mut tool = false;
    let mut terminal = 0;
    for frame in &frames {
        match protocol {
            0 => {
                text.push_str(
                    frame
                        .pointer("/choices/0/delta/content")
                        .and_then(Value::as_str)
                        .unwrap_or(""),
                );
                if let Some(calls) = frame
                    .pointer("/choices/0/delta/tool_calls")
                    .and_then(Value::as_array)
                {
                    for call in calls {
                        tool |= call["id"] == "call-a" && call["function"]["name"] == "lookup";
                        arguments.push_str(
                            call.pointer("/function/arguments")
                                .and_then(Value::as_str)
                                .unwrap_or(""),
                        );
                    }
                }
                if frame.pointer("/choices/0/finish_reason") == Some(&json!("tool_calls")) {
                    terminal += 1;
                    assert_eq!(frame["usage"]["total_tokens"], 7);
                }
            }
            1 => match frame["type"].as_str().unwrap_or("") {
                "content_block_start" => {
                    tool |= frame["content_block"]["type"] == "tool_use"
                        && frame["content_block"]["id"] == "call-a"
                        && frame["content_block"]["name"] == "lookup";
                }
                "content_block_delta" => {
                    text.push_str(
                        frame
                            .pointer("/delta/text")
                            .and_then(Value::as_str)
                            .unwrap_or(""),
                    );
                    arguments.push_str(
                        frame
                            .pointer("/delta/partial_json")
                            .and_then(Value::as_str)
                            .unwrap_or(""),
                    );
                }
                "message_stop" => terminal += 1,
                _ => {}
            },
            _ => match frame["type"].as_str().unwrap_or("") {
                "response.output_text.delta" => text.push_str(frame["delta"].as_str().unwrap()),
                "response.function_call_arguments.delta" => {
                    arguments.push_str(frame["delta"].as_str().unwrap())
                }
                "response.output_item.added" => {
                    tool |= frame["item"]["type"] == "function_call"
                        && frame["item"]["call_id"] == "call-a"
                        && frame["item"]["name"] == "lookup";
                }
                "response.completed" => terminal += 1,
                _ => {}
            },
        }
    }
    assert_eq!(text, "beforeafter");
    assert!(tool, "public tool identity was lost");
    assert_eq!(
        serde_json::from_str::<Value>(&arguments).unwrap(),
        json!({"v":"&quot;"})
    );
    assert_eq!(terminal, 1, "exactly one public success terminal");
    let terminal = frames
        .iter()
        .filter_map(|v| {
            let v = v.get("response").unwrap_or(v);
            v.get("gateway_bedrock_terminal").map(|_| v)
        })
        .collect::<Vec<_>>();
    assert_eq!(terminal.len(), 1);
    assert_eq!(
        terminal[0]["gateway_bedrock_terminal"]["events"],
        fixture::sequence().len()
    );
    terminal[0]["gateway_bedrock_turn"].clone()
}
