//! Public Bedrock request and response contracts, independent of private owners.

use neuro_gateway::protocol::bedrock_converse::{
    build_converse_success, default_path, normalize_converse, pack_bedrock_converse,
};
use neuro_gateway::protocol::canonical::{
    CanonicalToolCall, ContentPart, EndpointKind, MessageRole, ProtocolFamily, TokenUsage,
};
use serde_json::json;

#[test]
fn path_model_system_and_unknown_fields_survive_normalization() {
    let body = json!({
        "modelId": "body-model",
        "stream": false,
        "system": [{"text": "first"}, {"ignored": true}, {"text": "second"}],
        "messages": [{"role": "user", "content": [{"text": "hello"}]}],
        "inferenceConfig": {"maxTokens": 17},
        "requestMetadata": {"trace": "fixture"}
    });
    let request = normalize_converse(body.clone(), Some("path-model".into()), true).unwrap();
    assert_eq!(request.protocol_family, ProtocolFamily::BedrockConverse);
    assert_eq!(request.endpoint_kind, EndpointKind::ChatCompletions);
    assert_eq!(request.requested_model.as_deref(), Some("path-model"));
    assert!(request.stream);
    assert_eq!(request.raw_body, body);
    assert_eq!(request.messages.len(), 2);
    assert_eq!(request.messages[0].role, MessageRole::System);
    assert_eq!(request.messages[0].text_content(), "first\nsecond");
    assert_eq!(request.extra.len(), 2);
    assert_eq!(
        pack_bedrock_converse(&request, "upstream-model", true),
        json!({
            "modelId": "upstream-model",
            "stream": true,
            "system": [{"text": "first\nsecond"}],
            "messages": [{"role": "user", "content": [{"text": "hello"}]}],
            "inferenceConfig": {"maxTokens": 17},
            "requestMetadata": {"trace": "fixture"}
        })
    );
    assert_eq!(
        default_path("path-model", true),
        "/model/path-model/converse-stream"
    );
    assert_eq!(
        default_path("path-model", false),
        "/model/path-model/converse"
    );
}

#[test]
fn tool_use_and_result_keep_identity_json_and_tool_configuration() {
    let body = json!({
        "messages": [
            {"role": "assistant", "content": [
                {"text": "checking"},
                {"toolUse": {"toolUseId": "call-7", "name": "weather", "input": {"city": "Paris"}}}
            ]},
            {"role": "user", "content": [
                {"toolResult": {"toolUseId": "call-7", "content": [{"json": {"temperature": 18}}]}}
            ]}
        ],
        "toolConfig": {
            "tools": [{"toolSpec": {
                "name": "weather", "description": "Weather lookup",
                "inputSchema": {"json": {"type": "object"}}, "vendorField": "retained"
            }}],
            "toolChoice": {"tool": {"name": "weather"}}
        }
    });
    let request = normalize_converse(body, None, false).unwrap();
    assert_eq!(request.messages.len(), 2);
    assert_eq!(request.messages[0].role, MessageRole::Assistant);
    let call = &request.messages[0].tool_calls[0];
    assert_eq!(call.id.as_deref(), Some("call-7"));
    assert_eq!(call.name.as_deref(), Some("weather"));
    assert_eq!(call.arguments.as_deref(), Some("{\"city\":\"Paris\"}"));
    assert_eq!(request.messages[1].role, MessageRole::Tool);
    assert_eq!(request.messages[1].tool_call_id.as_deref(), Some("call-7"));
    assert_eq!(request.tools[0].raw["vendorField"], "retained");

    let packed = pack_bedrock_converse(&request, "model", false);
    assert_eq!(
        packed["messages"][0]["content"][1]["toolUse"],
        json!({
            "toolUseId": "call-7", "name": "weather", "input": {"city": "Paris"}
        })
    );
    assert_eq!(
        packed["messages"][1]["content"][0]["toolResult"],
        json!({
            "toolUseId": "call-7", "name": "weather", "status": "success",
            "content": [{"json": {"temperature": 18}}]
        })
    );
    assert_eq!(
        packed["toolConfig"]["toolChoice"],
        json!({"tool": {"name": "weather"}})
    );
    assert_eq!(
        packed["toolConfig"]["tools"][0]["toolSpec"]["inputSchema"],
        json!({"json": {"type": "object"}})
    );
    assert!(packed.get("stream").is_none());
}

#[test]
fn body_model_and_raw_content_are_preserved() {
    let raw = json!({"image": {"format": "png", "source": {"bytes": "fixture"}}});
    let body = json!({
        "modelId": "body-model",
        "messages": [
            {"role": "unknown", "content": [raw.clone(), {"json": {"structured": true}}]},
            {"role": "user", "content": []}
        ]
    });
    let request = normalize_converse(body, None, false).unwrap();
    assert_eq!(request.requested_model.as_deref(), Some("body-model"));
    assert_eq!(request.messages.len(), 1);
    assert_eq!(request.messages[0].role, MessageRole::User);
    assert!(matches!(&request.messages[0].content[0], ContentPart::Raw { value } if value == &raw));
    assert_eq!(
        pack_bedrock_converse(&request, "model", false)["messages"][0]["content"],
        json!([
            raw, {"json": {"structured": true}}
        ])
    );
}

#[test]
fn missing_or_non_array_messages_are_rejected() {
    for body in [
        json!({}),
        json!({"messages": null}),
        json!({"messages": {}}),
        json!({"messages": "bad"}),
    ] {
        let error = normalize_converse(body, None, false).unwrap_err();
        assert!(error
            .to_string()
            .contains("missing or invalid `messages` array"));
    }
}

#[test]
fn success_response_preserves_tool_input_usage_and_stop_reasons() {
    let calls = [CanonicalToolCall {
        id: Some("call-7".into()),
        call_type: "function".into(),
        name: Some("weather".into()),
        arguments: Some("{\"city\":\"Paris\"}".into()),
        raw: Default::default(),
    }];
    let usage = TokenUsage {
        prompt_tokens: 2,
        completion_tokens: 3,
        total_tokens: 5,
        cache_creation_input_tokens: None,
        cache_read_input_tokens: None,
    };
    assert_eq!(
        build_converse_success("model", "", Some(&usage), &calls, None),
        json!({
            "output": {"message": {"role": "assistant", "content": [{"toolUse": {
                "toolUseId": "call-7", "name": "weather", "input": {"city": "Paris"}
            }}]}},
            "model": "model", "stopReason": "tool_use",
            "usage": {"inputTokens": 2, "outputTokens": 3, "totalTokens": 5}
        })
    );
    for (reason, expected) in [
        ("stop", "end_turn"),
        ("length", "max_tokens"),
        ("content_filter", "guardrail_intervened"),
    ] {
        let result = build_converse_success("model", "answer", None, &[], Some(reason));
        assert_eq!(result["stopReason"], expected);
        assert_eq!(
            result["output"]["message"]["content"],
            json!([{"text": "answer"}])
        );
        assert!(result.get("usage").is_none());
    }
}
