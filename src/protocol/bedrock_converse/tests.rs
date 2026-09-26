use super::*;
use crate::protocol::canonical::{
    CanonicalMessage, CanonicalRelayRequest, CanonicalTool, CanonicalToolCall, ContentPart,
    MessageRole, ProtocolFamily,
};
use std::collections::HashMap;

fn base_request() -> CanonicalRelayRequest {
    CanonicalRelayRequest {
        protocol_family: ProtocolFamily::OpenAi,
        endpoint_kind: crate::protocol::canonical::EndpointKind::ChatCompletions,
        requested_model: Some("anthropic.claude-3-5-sonnet".to_string()),
        stream: false,
        messages: vec![CanonicalMessage {
            role: MessageRole::User,
            content: vec![ContentPart::Text {
                text: "hi".to_string(),
            }],
            name: None,
            tool_call_id: None,
            tool_calls: vec![],
        }],
        tools: vec![],
        tool_choice: None,
        reasoning: None,
        metadata: None,
        raw_body: json!({}),
        previous_response_id: None,
        explicit_session_key: None,
        extra: HashMap::new(),
    }
}

#[test]
fn packs_tool_specs_and_choice() {
    let mut req = base_request();
    req.tools.push(CanonicalTool {
        tool_type: "function".to_string(),
        name: Some("weather".to_string()),
        description: Some("Get weather".to_string()),
        input_schema: Some(json!({"type":"object"})),
        raw: HashMap::new(),
    });
    req.tool_choice = Some(json!("required"));
    let body = pack_bedrock_converse(&req, "anthropic.claude-3-5-sonnet", false);
    assert_eq!(
        body["toolConfig"]["tools"][0]["toolSpec"]["name"],
        "weather"
    );
    assert!(body["toolConfig"]["toolChoice"]["any"].is_object());
}

#[test]
fn packs_tool_result_as_user_tool_result_block() {
    let mut req = base_request();
    req.messages.push(CanonicalMessage {
        role: MessageRole::Assistant,
        content: vec![],
        name: None,
        tool_call_id: None,
        tool_calls: vec![CanonicalToolCall {
            id: Some("call_1".to_string()),
            call_type: "function".to_string(),
            name: Some("weather".to_string()),
            arguments: Some("{\"city\":\"Hangzhou\"}".to_string()),
            raw: HashMap::new(),
        }],
    });
    req.messages.push(CanonicalMessage {
        role: MessageRole::Tool,
        content: vec![ContentPart::Text {
            text: "{\"ok\":true}".to_string(),
        }],
        name: None,
        tool_call_id: Some("call_1".to_string()),
        tool_calls: vec![],
    });
    let body = pack_bedrock_converse(&req, "anthropic.claude-3-5-sonnet", false);
    assert_eq!(
        body["messages"][2]["content"][0]["toolResult"]["toolUseId"],
        "call_1"
    );
}

#[test]
fn maps_content_filter_to_guardrail_intervened() {
    assert_eq!(
        map_bedrock_finish_reason(Some("content_filter"), &[]),
        "guardrail_intervened"
    );
}
