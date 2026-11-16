//! Native signed history must retain the original ordered prefix, not a flat text/tool projection.
use neuro_gateway::protocol::{
    bedrock_converse::{normalize_converse, pack_bedrock_converse, try_pack_bedrock_converse},
    canonical::ContentPart,
};
use serde_json::{json, Value};

#[path = "bedrock_signed_history_contract/wire.rs"]
mod wire;

fn history() -> Value {
    json!({"system":[{"text":"system A "},{"text":"system B"}],
        "messages":[
            {"role":"user","content":[{"text":" question\r\n"}]},
            {"role":"assistant","content":[
                {"reasoningContent":{"reasoningText":{"text":" thinking\r\n ","signature":"synthetic-signature.=="}}},
                {"text":"before "},
                {"toolUse":{"toolUseId":"call","name":"lookup","input":{"city":"杭州"}}},
                {"text":" after"},
                {"reasoningContent":{"redactedContent":"AP8="}}
            ]},
            {"role":"user","content":[
                {"toolResult":{"toolUseId":"call","status":"success","content":[{"json":{"ok":true}}]}},
                {"text":"next"}
            ]}
        ],"toolConfig":{"tools":[{"toolSpec":{"name":"lookup","inputSchema":{"json":{"type":"object"}}}}]},
        "inferenceConfig":{"maxTokens":17}})
}

#[test]
fn bedrock_signed_history_preserves_order_system_and_opaque_values_in_both_packers() {
    let body = history();
    for stream in [false, true] {
        let req = normalize_converse(body.clone(), Some("fixture".into()), stream).unwrap();
        for packed in [
            pack_bedrock_converse(&req, "fixture", stream),
            try_pack_bedrock_converse(&req, "fixture", stream).unwrap(),
        ] {
            assert_eq!(packed["messages"], body["messages"]);
            assert_eq!(packed["system"], body["system"]);
            assert_eq!(packed["inferenceConfig"]["maxTokens"], 17);
        }
    }
}

#[test]
fn bedrock_signed_history_rejects_policy_changed_messages_instead_of_replaying_raw() {
    for system in [false, true] {
        let mut req = normalize_converse(history(), Some("fixture".into()), false).unwrap();
        let index = if system { 0 } else { 2 };
        req.messages[index].content.push(ContentPart::Text {
            text: "policy edit".into(),
        });
        let error = try_pack_bedrock_converse(&req, "fixture", false).unwrap_err();
        assert_eq!(
            error.code.as_deref(),
            Some("bedrock_reasoning_history_changed")
        );
    }
}

#[test]
fn bedrock_signed_history_rejects_malformed_unions_without_requiring_optional_signature() {
    for invalid in [
        json!(null),
        json!({}),
        json!({"futureReasoning":"opaque"}),
        json!({"reasoningText":false}),
        json!({"reasoningText":{}}),
        json!({"reasoningText":{"text":false}}),
        json!({"reasoningText":{"text":"x","signature":null}}),
        json!({"reasoningText":{"text":"x"},"redactedContent":"AP8="}),
        json!({"redactedContent":"not-base64!"}),
        json!({"redactedContent":12}),
    ] {
        let mut body = history();
        body["messages"][1]["content"][0]["reasoningContent"] = invalid;
        assert!(normalize_converse(body, Some("fixture".into()), false).is_err());
    }
    let mut body = history();
    body["messages"][1]["content"][0]["text"] = json!("competing member");
    assert!(normalize_converse(body, Some("fixture".into()), false).is_err());
    for valid in [
        json!({"reasoningText":{"text":""}}),
        json!({"redactedContent":""}),
    ] {
        let mut body = history();
        body["messages"][1]["content"][0]["reasoningContent"] = valid;
        let req = normalize_converse(body.clone(), Some("fixture".into()), false).unwrap();
        assert_eq!(
            try_pack_bedrock_converse(&req, "fixture", false).unwrap()["messages"],
            body["messages"]
        );
    }
}

#[test]
fn bedrock_signed_history_rejects_tool_role_and_prefix_edits_without_raw_policy_bypass() {
    for edit in 0..3 {
        let mut req = normalize_converse(history(), Some("fixture".into()), false).unwrap();
        match edit {
            0 => req.messages[2].tool_calls[0].arguments = Some("{\"edited\":true}".into()),
            1 => req.messages[1].role = neuro_gateway::protocol::canonical::MessageRole::Assistant,
            _ => {
                req.messages.remove(1);
            }
        }
        assert_eq!(
            try_pack_bedrock_converse(&req, "fixture", false)
                .unwrap_err()
                .code
                .as_deref(),
            Some("bedrock_reasoning_history_changed")
        );
        // Structural compatibility has no validation result, but cannot replay raw over edits.
        assert_ne!(
            pack_bedrock_converse(&req, "fixture", false)["messages"],
            history()["messages"]
        );
    }
}

#[test]
fn bedrock_reasoning_history_with_absent_system_and_optional_signature_stays_exact() {
    let body = json!({"messages":[{"role":"assistant","content":[
        {"reasoningContent":{"reasoningText":{"text":""}}},{"text":"answer"}]}]});
    let req = normalize_converse(body.clone(), Some("fixture".into()), false).unwrap();
    let packed = try_pack_bedrock_converse(&req, "fixture", false).unwrap();
    assert_eq!(packed["messages"], body["messages"]);
    assert!(packed.get("system").is_none());
}

#[test]
fn bedrock_signed_history_cannot_be_replaced_by_extra_messages_or_system() {
    for key in ["messages", "system"] {
        let mut req = normalize_converse(history(), Some("fixture".into()), false).unwrap();
        req.extra.insert(key.into(), json!([]));
        assert_eq!(
            try_pack_bedrock_converse(&req, "fixture", false)
                .unwrap_err()
                .code
                .as_deref(),
            Some("bedrock_reasoning_history_changed")
        );
    }
}

#[test]
fn bedrock_unsigned_history_still_honors_canonical_policy_edits() {
    let mut req = normalize_converse(
        json!({"messages":[{"role":"user","content":[{"text":"original"}]}]}),
        Some("fixture".into()),
        false,
    )
    .unwrap();
    req.messages[0].content = vec![ContentPart::Text {
        text: "filtered".into(),
    }];
    assert_eq!(
        try_pack_bedrock_converse(&req, "fixture", false).unwrap()["messages"][0]["content"][0]
            ["text"],
        "filtered"
    );
}
