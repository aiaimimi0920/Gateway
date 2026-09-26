use super::*;
use crate::protocol::canonical::{CanonicalRelayRequest, EndpointKind, ProtocolFamily};
use serde_json::json;

// ── Helpers ──────────────────────────────────────────────────────────

fn make_tool(name: &str, desc: &str, schema: Value) -> CanonicalTool {
    CanonicalTool {
        tool_type: "function".to_string(),
        name: Some(name.to_string()),
        description: Some(desc.to_string()),
        input_schema: Some(schema),
        raw: HashMap::new(),
    }
}

fn make_request_with_tools(tools: Vec<CanonicalTool>) -> CanonicalRelayRequest {
    CanonicalRelayRequest {
        protocol_family: ProtocolFamily::OpenAi,
        endpoint_kind: EndpointKind::ChatCompletions,
        requested_model: Some("deepseek-chat".to_string()),
        stream: false,
        messages: vec![
            CanonicalMessage {
                role: MessageRole::System,
                content: vec![ContentPart::Text {
                    text: "You are a helpful assistant.".to_string(),
                }],
                name: None,
                tool_call_id: None,
                tool_calls: vec![],
            },
            CanonicalMessage {
                role: MessageRole::User,
                content: vec![ContentPart::Text {
                    text: "Read the file /tmp/test.txt".to_string(),
                }],
                name: None,
                tool_call_id: None,
                tool_calls: vec![],
            },
        ],
        tools,
        tool_choice: Some(json!("auto")),
        reasoning: None,
        metadata: None,
        raw_body: json!({}),
        previous_response_id: None,
        explicit_session_key: None,
        extra: HashMap::new(),
    }
}

mod history;
mod parser;
mod policy;
mod prompt;
mod streaming;
