use std::collections::HashMap;

use serde_json::{json, Value};

use super::pack_kiro;
use crate::protocol::canonical::{
    CanonicalMessage, CanonicalRelayRequest, CanonicalTool, CanonicalToolCall, ContentPart,
    EndpointKind, MessageRole, ProtocolFamily,
};

fn message(role: MessageRole, content: Vec<ContentPart>) -> CanonicalMessage {
    CanonicalMessage {
        role,
        content,
        name: None,
        tool_call_id: None,
        tool_calls: Vec::new(),
    }
}

fn text_message(role: MessageRole, text: &str) -> CanonicalMessage {
    message(
        role,
        vec![ContentPart::Text {
            text: text.to_string(),
        }],
    )
}

fn request(messages: Vec<CanonicalMessage>) -> CanonicalRelayRequest {
    CanonicalRelayRequest {
        protocol_family: ProtocolFamily::OpenAi,
        endpoint_kind: EndpointKind::ChatCompletions,
        requested_model: Some("claude-sonnet-4-20250514".to_string()),
        stream: false,
        messages,
        tools: Vec::new(),
        tool_choice: None,
        reasoning: None,
        metadata: None,
        raw_body: json!({}),
        previous_response_id: None,
        explicit_session_key: Some("session-123".to_string()),
        extra: HashMap::new(),
    }
}

#[test]
fn pack_kiro_preserves_missing_message_and_image_errors() {
    let missing = request(vec![text_message(MessageRole::System, "system only")]);
    let error = pack_kiro(&missing, "claude-sonnet-4.6").expect_err("turn is required");
    assert_eq!(error.http_status, Some(400));
    assert_eq!(error.code.as_deref(), Some("kiro_missing_messages"));
    assert_eq!(
        error.message,
        "Kiro requests require at least one non-system user/tool turn"
    );

    let unsupported = request(vec![message(
        MessageRole::User,
        vec![ContentPart::ImageUrl {
            image_url: "data:image/svg+xml;base64,AAAA".to_string(),
            detail: None,
        }],
    )]);
    let error =
        pack_kiro(&unsupported, "claude-sonnet-4.6").expect_err("unsupported image type must fail");
    assert_eq!(error.http_status, Some(400));
    assert_eq!(
        error.code.as_deref(),
        Some("kiro_unsupported_image_media_type")
    );
    assert_eq!(
        error.message,
        "Unsupported Kiro image media type: image/svg+xml"
    );

    let invalid = request(vec![message(
        MessageRole::User,
        vec![ContentPart::ImageUrl {
            image_url: "data:image/png;base64,%%%".to_string(),
            detail: None,
        }],
    )]);
    let error = pack_kiro(&invalid, "claude-sonnet-4.6").expect_err("invalid base64 must fail");
    assert_eq!(error.http_status, Some(400));
    assert_eq!(error.code.as_deref(), Some("kiro_invalid_image_base64"));
    assert!(error
        .message
        .starts_with("Invalid Kiro image base64 payload:"));
}

#[test]
fn pack_kiro_preserves_content_session_extra_and_raw_body_boundaries() {
    let mut req = request(vec![message(
        MessageRole::User,
        vec![
            ContentPart::Text {
                text: "hello".to_string(),
            },
            ContentPart::Json {
                value: json!({"json": 1}),
            },
            ContentPart::Raw {
                value: json!(["raw"]),
            },
            ContentPart::ImageUrl {
                image_url: "data:image/png;base64,aGk=".to_string(),
                detail: Some("high".to_string()),
            },
        ],
    )]);
    req.raw_body = json!({"raw_only": "must-not-be-forwarded"});
    req.metadata = Some(HashMap::from([(
        "user_id".to_string(),
        Value::String("metadata-session".to_string()),
    )]));
    req.explicit_session_key = Some("explicit-session".to_string());
    req.extra
        .insert("vendorExtension".to_string(), json!({"enabled": true}));
    req.extra
        .insert("model".to_string(), Value::String("must-skip".to_string()));

    let packed = pack_kiro(&req, "claude-sonnet-4.6").expect("request packs");
    let state = &packed["conversationState"];
    let current = &state["currentMessage"]["userInputMessage"];
    assert_eq!(state["conversationId"], "explicit-session");
    assert_eq!(current["content"], "hello\n{\"json\":1}\n[\"raw\"]");
    assert_eq!(current["images"][0]["format"], "png");
    assert_eq!(current["images"][0]["source"]["bytes"], "aGk=");
    assert_eq!(state["vendorExtension"], json!({"enabled": true}));
    assert!(state.get("model").is_none());
    assert!(state.get("raw_only").is_none());

    req.explicit_session_key = None;
    let packed = pack_kiro(&req, "claude-sonnet-4.6").expect("metadata session packs");
    assert_eq!(
        packed["conversationState"]["conversationId"],
        "metadata-session"
    );
}

#[test]
fn pack_kiro_preserves_literal_and_percent_encoded_base64_plus() {
    for image_url in [
        "data:image/png;base64,+w==",
        "data:image/png;base64,%2Bw%3D%3D",
        "data:IMAGE/PNG;BASE64,+w==",
    ] {
        let req = request(vec![message(
            MessageRole::User,
            vec![ContentPart::ImageUrl {
                image_url: image_url.to_string(),
                detail: None,
            }],
        )]);

        let packed = pack_kiro(&req, "claude-sonnet-4.6").expect("valid base64 image packs");
        assert_eq!(
            packed["conversationState"]["currentMessage"]["userInputMessage"]["images"][0]
                ["source"]["bytes"],
            "+w=="
        );
    }
}

#[test]
fn pack_kiro_preserves_tool_name_mapping_pairing_and_order() {
    let long_name = format!("read_{}", "workspace_tree_".repeat(6));
    let mut assistant = text_message(MessageRole::Assistant, "calling tool");
    assistant.tool_calls.push(CanonicalToolCall {
        id: Some("call-1".to_string()),
        call_type: "function".to_string(),
        name: Some(long_name.clone()),
        arguments: Some("{\"path\":\"src\"}".to_string()),
        raw: HashMap::new(),
    });
    let mut tool_result = message(
        MessageRole::Tool,
        vec![ContentPart::Json {
            value: json!({"entries": 3}),
        }],
    );
    tool_result.tool_call_id = Some("call-1".to_string());

    let mut req = request(vec![
        text_message(MessageRole::User, "inspect"),
        assistant,
        tool_result,
        text_message(MessageRole::User, "continue"),
    ]);
    req.tools.push(CanonicalTool {
        tool_type: "function".to_string(),
        name: Some(long_name.clone()),
        description: Some("Read workspace tree".to_string()),
        input_schema: Some(json!({"type": "object"})),
        raw: HashMap::new(),
    });

    let packed = pack_kiro(&req, "claude-sonnet-4.6").expect("tool request packs");
    let state = &packed["conversationState"];
    let declared_tools = state["currentMessage"]["userInputMessage"]["userInputMessageContext"]
        ["tools"]
        .as_array()
        .expect("declared tools");
    assert_eq!(declared_tools.len(), 1);
    let declared_name = declared_tools[0]["toolSpecification"]["name"]
        .as_str()
        .expect("declared tool name");
    let history_use = &state["history"][1]["assistantResponseMessage"]["toolUses"][0];
    let current = &state["currentMessage"]["userInputMessage"];
    assert!(declared_name.chars().count() <= 63);
    assert_ne!(declared_name, long_name);
    assert_eq!(history_use["name"], declared_name);
    assert_eq!(history_use["toolUseId"], "call-1");
    assert_eq!(history_use["input"], json!({"path": "src"}));
    assert_eq!(current["content"], "continue");
    assert_eq!(
        current["userInputMessageContext"]["toolResults"][0]["toolUseId"],
        "call-1"
    );
}

#[test]
fn pack_kiro_drops_orphan_tool_results_without_reordering_text() {
    let mut orphan = text_message(MessageRole::Tool, "orphan");
    orphan.tool_call_id = Some("missing-call".to_string());
    let req = request(vec![
        text_message(MessageRole::User, "first"),
        orphan,
        text_message(MessageRole::User, "last"),
    ]);

    let packed = pack_kiro(&req, "claude-sonnet-4.6").expect("request packs");
    let current = &packed["conversationState"]["currentMessage"]["userInputMessage"];
    assert_eq!(current["content"], "first\nlast");
    assert_eq!(current["userInputMessageContext"]["toolResults"], json!([]));
}
