use super::wire;
use neuro_gateway::{
    error::GatewayError,
    protocol::{anthropic, canonical::CanonicalRelayResponse, openai, responses},
};
use serde_json::{json, Value};

pub fn content() -> Value {
    json!([
        {"text":"before"},
        {"reasoningContent":{"reasoningText":{"text":"private-reasoning\r\n", "signature":" private-signature \r\n"}}},
        {"toolUse":{"toolUseId":"call-a","name":"lookup","input":{"v":"&quot;"}}},
        {"reasoningContent":{"redactedContent":"AQIDBA=="}},
        {"text":"after"}
    ])
}
pub fn reply(content: Value) -> Value {
    let stop = if content
        .as_array()
        .unwrap()
        .iter()
        .any(|block| block.get("toolUse").is_some())
    {
        "tool_use"
    } else {
        "end_turn"
    };
    json!({"output":{"message":{"role":"assistant","content":content}},"stopReason":stop,
        "usage":{"inputTokens":3,"outputTokens":4,"totalTokens":7}})
}
pub fn sequence() -> Vec<Vec<u8>> {
    let mut events = vec![wire::event("messageStart", json!({"role":"assistant"}))];
    for (index, block) in content().as_array().unwrap().iter().enumerate() {
        if let Some(tool) = block.get("toolUse") {
            events.push(wire::event("contentBlockStart", json!({"contentBlockIndex":index,"start":{"toolUse":{"toolUseId":tool["toolUseId"],"name":tool["name"]}}})));
        }
        let deltas = if let Some(text) = block.get("text") {
            vec![json!({"text":text})]
        } else if let Some(tool) = block.get("toolUse") {
            vec![json!({"toolUse":{"input":tool["input"].to_string()}})]
        } else if let Some(text) = block.pointer("/reasoningContent/reasoningText") {
            vec![
                json!({"reasoningContent":{"text":text["text"]}}),
                json!({"reasoningContent":{"signature":text["signature"]}}),
            ]
        } else {
            // Both fragments have padding; concatenating Base64 strings is invalid.
            vec![
                json!({"reasoningContent":{"redactedContent":"AQ=="}}),
                json!({"reasoningContent":{"redactedContent":"AgME"}}),
            ]
        };
        for delta in deltas {
            events.push(wire::event(
                "contentBlockDelta",
                json!({"contentBlockIndex":index,"delta":delta}),
            ));
        }
        events.push(wire::event(
            "contentBlockStop",
            json!({"contentBlockIndex":index}),
        ));
    }
    events.push(wire::event("messageStop", json!({"stopReason":"tool_use"})));
    events.push(wire::event(
        "metadata",
        json!({"usage":{"inputTokens":3,"outputTokens":4,"totalTokens":7}}),
    ));
    events
}

pub type Reader = fn(&Value) -> Result<CanonicalRelayResponse, GatewayError>;
pub fn public_body(protocol: u8) -> (Value, Reader) {
    let call = neuro_gateway::protocol::canonical::CanonicalToolCall {
        id: Some("call-a".into()),
        name: Some("lookup".into()),
        call_type: "function".into(),
        arguments: Some(json!({"v":"&quot;"}).to_string()),
        raw: Default::default(),
    };
    match protocol {
        0 => (
            openai::build_chat_completions_success(
                "test",
                0,
                "alias",
                "beforeafter",
                None,
                &[call],
                Some("tool_calls"),
            ),
            openai::unpack_openai_response,
        ),
        1 => (
            anthropic::build_messages_success(
                "test",
                "alias",
                "beforeafter",
                None,
                &[call],
                Some("tool_calls"),
            ),
            anthropic::unpack_anthropic_response,
        ),
        _ => (
            responses::build_responses_success(
                "test",
                "alias",
                "beforeafter",
                None,
                &[call],
                Some("tool_calls"),
            ),
            responses::unpack_responses_response,
        ),
    }
}
