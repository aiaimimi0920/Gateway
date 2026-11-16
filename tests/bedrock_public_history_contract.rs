//! Explicit Bedrock continuation: public protocols carry native reasoning without interpreting signatures.
#![cfg(feature = "family-bedrock-converse-official-api")]
#[path = "bedrock_public_history_contract/adversarial.rs"]
mod adversarial;
#[path = "bedrock_reasoning_response_contract/fixtures.rs"]
mod fixture;
#[path = "bedrock_public_history_contract/routes.rs"]
mod routes;
#[path = "console_contract_support/mod.rs"]
mod support;
#[path = "bedrock_upstream_eventstream_contract/fixtures.rs"]
mod wire;
use neuro_gateway::protocol::{
    anthropic, bedrock_converse,
    canonical::{CanonicalRelayRequest, ContentPart},
    openai, responses,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

fn normalize(protocol: u8, body: Value) -> CanonicalRelayRequest {
    match protocol {
        0 => openai::normalize_chat_completions(body),
        1 => anthropic::normalize_messages(body),
        _ => responses::normalize_responses(body),
    }
    .unwrap()
}
fn first(protocol: u8) -> Value {
    let mut body = if protocol == 2 {
        json!({"model":"fixture","input":"hello"})
    } else {
        json!({"model":"fixture","messages":[{"role":"user","content":"hello"}]})
    };
    body["tools"] = match protocol {
        0 => {
            json!([{"type":"function","function":{"name":"lookup","parameters":{"type":"object"}}}])
        }
        1 => json!([{"name":"lookup","input_schema":{"type":"object"}}]),
        _ => json!([{"type":"function","name":"lookup","parameters":{"type":"object"}}]),
    };
    body
}
fn scope(protocol: u8) -> Value {
    let body = bedrock_converse::try_pack_bedrock_converse(
        &normalize(protocol, first(protocol)),
        "fixture",
        false,
    )
    .unwrap();
    let prefix = json!({"messages":body["messages"],"system":body.get("system")});
    json!({"source":"aws_bedrock_converse","model":"fixture","request_sha256":format!("{:x}",Sha256::digest(serde_json::to_vec(&prefix).unwrap()))})
}
fn turn(protocol: u8) -> Value {
    json!({"content":fixture::content(),"continuation":scope(protocol)})
}
fn followup(protocol: u8, turn: Value) -> Value {
    let mut body = match protocol {
        0 => {
            json!({"model":"fixture","messages":[{"role":"user","content":"hello"},{"role":"assistant","content":"beforeafter","tool_calls":[{"id":"call-a","type":"function","function":{"name":"lookup","arguments":"{\"v\":\"&quot;\"}"}}]},{"role":"tool","tool_call_id":"call-a","content":"done"}]})
        }
        1 => {
            json!({"model":"fixture","messages":[{"role":"user","content":"hello"},{"role":"assistant","content":[{"type":"text","text":"beforeafter"},{"type":"tool_use","id":"call-a","name":"lookup","input":{"v":"&quot;"}}]},{"role":"user","content":[{"type":"tool_result","tool_use_id":"call-a","content":"done"}]}]})
        }
        _ => {
            json!({"model":"fixture","input":[{"role":"user","content":"hello"},{"role":"assistant","content":[{"type":"output_text","text":"beforeafter"}]},{"type":"function_call","call_id":"call-a","name":"lookup","arguments":"{\"v\":\"&quot;\"}"},{"type":"function_call_output","call_id":"call-a","output":"done"}]})
        }
    };
    body["gateway_bedrock_history"] = json!([{"assistant_turn":0,"turn":turn}]);
    body["tools"] = first(protocol)["tools"].clone();
    body
}

#[test]
fn public_history_restores_native_order_and_literal_tool_arguments_in_three_protocols() {
    for protocol in 0..3 {
        let req = normalize(protocol, followup(protocol, turn(protocol)));
        let body = bedrock_converse::try_pack_bedrock_converse(&req, "fixture", false).unwrap();
        assert_eq!(body["messages"][1]["content"], fixture::content());
        assert!(body.get("gateway_bedrock_history").is_none());
        assert_eq!(
            body["messages"][2]["content"][0]["toolResult"],
            json!({"toolUseId":"call-a","content":[{"text":"done"}]})
        );
    }
}
#[test]
fn public_history_cannot_change_the_resolved_upstream_model() {
    let req = normalize(0, followup(0, turn(0)));
    assert!(bedrock_converse::try_pack_bedrock_converse(&req, "other-model", false).is_err());
}
#[test]
fn public_history_cannot_restore_over_policy_changes_or_extra_overrides() {
    for mode in 0..3 {
        let mut req = normalize(0, followup(0, turn(0)));
        match mode {
            0 => {
                req.messages[0].content = vec![ContentPart::Text {
                    text: "changed".into(),
                }]
            }
            1 => {
                req.extra.insert("messages".into(), json!([]));
            }
            _ => {
                req.extra.insert("system".into(), json!([]));
            }
        }
        assert!(bedrock_converse::try_pack_bedrock_converse(&req, "fixture", false).is_err());
    }
}
#[test]
fn old_unbound_response_turn_is_not_a_public_continuation() {
    let req = normalize(0, followup(0, json!({"content":fixture::content()})));
    assert!(bedrock_converse::try_pack_bedrock_converse(&req, "fixture", false).is_err());
}
