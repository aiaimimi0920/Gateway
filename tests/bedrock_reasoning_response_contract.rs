//! Buffered response fidelity, using synthetic AWS bytes and no live credentials.
#![cfg(feature = "family-bedrock-converse-official-api")]
#[path = "bedrock_reasoning_response_contract/adversarial.rs"]
mod adversarial;
#[path = "bedrock_reasoning_response_contract/fixtures.rs"]
mod fixtures;
#[path = "bedrock_upstream_eventstream_contract/fixtures.rs"]
mod wire;

use fixtures::*;
use neuro_gateway::protocol::{
    bedrock_converse::unpack_bedrock_response, canonical::CanonicalRelayResponse,
};
use serde_json::{json, Value};

#[test]
fn native_json_retains_reasoning_order_and_opaque_values_outside_visible_text() {
    let content = content();
    let result = unpack_bedrock_response(&reply(content.clone())).unwrap();
    assert_eq!(result.text, "beforeafter");
    assert_eq!(
        result.tool_calls[0].arguments.as_deref(),
        Some("{\"v\":\"&quot;\"}")
    );
    assert_eq!(
        serde_json::to_value(&result).unwrap()["bedrock_turn"]["content"],
        content
    );
    let debug = format!("{result:?}");
    assert!(!debug.contains("private-signature"));
    assert!(!debug.contains("private-reasoning"));
}

#[test]
fn native_json_accepts_optional_signature_and_empty_reasoning_values() {
    for reasoning in [
        json!({"reasoningText":{"text":""}}),
        json!({"reasoningText":{"text":"","signature":""}}),
        json!({"redactedContent":""}),
    ] {
        let value = json!([{"reasoningContent":reasoning}]);
        let result = unpack_bedrock_response(&reply(value.clone())).unwrap();
        assert_eq!(
            serde_json::to_value(result).unwrap()["bedrock_turn"]["content"],
            value
        );
    }
}

#[test]
fn malformed_native_reasoning_is_not_accepted_as_success() {
    for value in [
        json!({"reasoningText":{"text":"a"},"redactedContent":""}),
        json!({"reasoningText":{"signature":"s"}}),
        json!({"reasoningText":{"text":"a","signature":null}}),
        json!({"redactedContent":"%%%"}),
        json!({"future":"x"}),
    ] {
        assert!(unpack_bedrock_response(&reply(json!([{"reasoningContent":value}]))).is_err());
    }
}

#[test]
fn native_json_tool_stop_requires_a_real_tool_identity() {
    let mut body = reply(json!([{"reasoningContent":{"reasoningText":{"text":"x"}}}]));
    body["stopReason"] = json!("tool_use");
    assert!(unpack_bedrock_response(&body).is_err());
}

#[tokio::test]
async fn buffered_aws_frames_decode_redacted_fragments_as_bytes_not_base64_text() {
    let result = neuro_gateway::upstream::bedrock_converse_official_api_common::accumulate_stream(
        wire::response(sequence()),
        "fixture-model",
    )
    .await
    .unwrap();
    assert_eq!(result.text, "beforeafter");
    assert_eq!(
        serde_json::to_value(result).unwrap()["bedrock_turn"]["content"],
        content()
    );
}

#[test]
fn public_json_readers_retain_the_turn_and_reject_projection_mutation() {
    for protocol in 0..3 {
        let (mut body, read) = public_body(protocol);
        body["gateway_bedrock_turn"] = json!({"content":content()});
        let result = read(&body).unwrap();
        assert_eq!(
            serde_json::to_value(result).unwrap()["bedrock_turn"]["content"],
            content()
        );
        body["gateway_bedrock_turn"]["content"][0]["text"] = json!("forged");
        assert!(read(&body).is_err());
    }
}

#[test]
fn public_json_rejects_null_oversized_and_duplicate_tool_turns() {
    let (mut body, read) = public_body(0);
    for value in [
        Value::Null,
        json!({"content":[{"reasoningContent":{"reasoningText":{"text":"x".repeat(4*1024*1024)}}}]}),
        json!({"content":[{"reasoningContent":{"reasoningText":{"text":""}}}, {"toolUse":{"toolUseId":"x","name":"f","input":{}}}, {"toolUse":{"toolUseId":"x","name":"f","input":{}}}]}),
    ] {
        body["gateway_bedrock_turn"] = value;
        assert!(read(&body).is_err());
    }
}

#[test]
fn old_canonical_json_has_no_new_field_when_absent() {
    let result = unpack_bedrock_response(&reply(json!([{"text":"OK"}]))).unwrap();
    let value = serde_json::to_value(&result).unwrap();
    assert!(value.get("bedrock_turn").is_none());
    let old: CanonicalRelayResponse = serde_json::from_value(value).unwrap();
    assert_eq!(old.text, "OK");
}
