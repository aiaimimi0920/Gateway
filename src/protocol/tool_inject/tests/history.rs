use super::*;

// ── inject_tools ────────────────────────────────────────────────────

#[test]
fn inject_tools_modifies_request() {
    let tools = vec![make_tool(
        "read_file",
        "Read a file",
        json!({
            "type": "object",
            "properties": {
                "path": {"type": "string", "description": "File path"}
            },
            "required": ["path"]
        }),
    )];

    let mut req = make_request_with_tools(tools);
    assert!(!req.tools.is_empty());
    assert!(req.tool_choice.is_some());

    inject_tools(&mut req);

    // Tools and tool_choice should be cleared.
    assert!(req.tools.is_empty());
    assert!(req.tool_choice.is_none());

    // System message should contain the tool prompt.
    let system_text = req
        .messages
        .iter()
        .find(|m| m.role == MessageRole::System)
        .unwrap()
        .text_content();
    assert!(system_text.contains("<tools>"));
    assert!(system_text.contains("read_file"));
    assert!(system_text.contains("TOOL CALL FORMAT"));
    // Original system message should still be present.
    assert!(system_text.contains("You are a helpful assistant."));
}

#[test]
fn inject_tools_creates_system_when_absent() {
    let tools = vec![make_tool(
        "test_tool",
        "A test tool",
        json!({"type": "object", "properties": {}}),
    )];

    let mut req = CanonicalRelayRequest {
        protocol_family: ProtocolFamily::OpenAi,
        endpoint_kind: EndpointKind::ChatCompletions,
        requested_model: Some("deepseek-chat".to_string()),
        stream: false,
        messages: vec![CanonicalMessage {
            role: MessageRole::User,
            content: vec![ContentPart::Text {
                text: "Hello".to_string(),
            }],
            name: None,
            tool_call_id: None,
            tool_calls: vec![],
        }],
        tools,
        tool_choice: None,
        reasoning: None,
        metadata: None,
        raw_body: json!({}),
        previous_response_id: None,
        explicit_session_key: None,
        extra: HashMap::new(),
    };

    inject_tools(&mut req);

    // A system message should be inserted at position 0.
    assert_eq!(req.messages[0].role, MessageRole::System);
    assert!(req.messages[0].text_content().contains("<tools>"));
    // User message should still be at position 1.
    assert_eq!(req.messages[1].role, MessageRole::User);
}

#[test]
fn inject_tools_noop_when_no_tools() {
    let mut req = make_request_with_tools(vec![]);
    let original_msg_count = req.messages.len();
    inject_tools(&mut req);
    assert_eq!(req.messages.len(), original_msg_count);
}

#[test]
fn inject_tools_converts_tool_history() {
    let tools = vec![make_tool(
        "read_file",
        "Read a file",
        json!({"type": "object", "properties": {}}),
    )];

    let mut req = make_request_with_tools(tools);

    // Add assistant message with tool_calls.
    req.messages.push(CanonicalMessage {
        role: MessageRole::Assistant,
        content: vec![],
        name: None,
        tool_call_id: None,
        tool_calls: vec![CanonicalToolCall {
            id: Some("call_123".to_string()),
            call_type: "function".to_string(),
            name: Some("read_file".to_string()),
            arguments: Some("{\"path\":\"/tmp/test.txt\"}".to_string()),
            raw: HashMap::new(),
        }],
    });

    // Add tool result message.
    req.messages.push(CanonicalMessage {
        role: MessageRole::Tool,
        content: vec![ContentPart::Text {
            text: "file contents here".to_string(),
        }],
        name: None,
        tool_call_id: Some("call_123".to_string()),
        tool_calls: vec![],
    });

    inject_tools(&mut req);

    // Assistant message should now have XML tool calls in text.
    let assistant_msg = &req.messages[2];
    assert_eq!(assistant_msg.role, MessageRole::Assistant);
    let assistant_text = assistant_msg.text_content();
    assert!(assistant_text.contains("<tool_calls>"));
    assert!(assistant_text.contains("read_file"));
    assert!(assistant_msg.tool_calls.is_empty());

    // Tool result should be converted to user role with XML formatting.
    let tool_msg = &req.messages[3];
    assert_eq!(tool_msg.role, MessageRole::User);
    let tool_text = tool_msg.text_content();
    assert!(tool_text.contains("<tool_result>"));
    assert!(tool_text.contains("file contents here"));
    assert!(tool_text.contains("call_123"));
    assert!(tool_msg.tool_call_id.is_none());
}

#[test]
fn serialize_tool_history_preserves_json_tool_result_content_without_tools() {
    let mut req = CanonicalRelayRequest {
        protocol_family: ProtocolFamily::OpenAi,
        endpoint_kind: EndpointKind::ChatCompletions,
        requested_model: Some("qwen".to_string()),
        stream: false,
        messages: vec![
            CanonicalMessage {
                role: MessageRole::Assistant,
                content: vec![],
                name: None,
                tool_call_id: None,
                tool_calls: vec![CanonicalToolCall {
                    id: Some("call_weather".to_string()),
                    call_type: "function".to_string(),
                    name: Some("weather".to_string()),
                    arguments: Some("{\"city\":\"Hangzhou\"}".to_string()),
                    raw: HashMap::new(),
                }],
            },
            CanonicalMessage {
                role: MessageRole::Tool,
                content: vec![ContentPart::Json {
                    value: json!({"city":"Hangzhou","condition":"sunny"}),
                }],
                name: Some("weather".to_string()),
                tool_call_id: Some("call_weather".to_string()),
                tool_calls: vec![],
            },
        ],
        tools: vec![],
        tool_choice: None,
        reasoning: None,
        metadata: None,
        raw_body: json!({}),
        previous_response_id: None,
        explicit_session_key: None,
        extra: HashMap::new(),
    };

    serialize_tool_history(&mut req);
    assert_eq!(req.messages[0].role, MessageRole::Assistant);
    assert!(req.messages[0].text_content().contains("<tool_calls>"));
    assert_eq!(req.messages[1].role, MessageRole::User);
    let rendered = req.messages[1].text_content();
    assert!(rendered.contains("<tool_result>"));
    assert!(rendered.contains("\"condition\":\"sunny\""));
}

// ── format_tool_call_as_xml ─────────────────────────────────────────

#[test]
fn format_tool_call_xml_valid() {
    let calls = vec![CanonicalToolCall {
        id: Some("call_1".to_string()),
        call_type: "function".to_string(),
        name: Some("test_tool".to_string()),
        arguments: Some("{\"key\":\"value\"}".to_string()),
        raw: HashMap::new(),
    }];

    let xml = format_tool_call_as_xml(&calls);
    assert!(xml.contains("<tool_calls>"));
    assert!(xml.contains("</tool_calls>"));
    assert!(xml.contains("<tool_name>test_tool</tool_name>"));
    assert!(xml.contains("<parameters>"));
}

// ── xml_escape ──────────────────────────────────────────────────────

#[test]
fn xml_escape_special_chars() {
    assert_eq!(xml_escape("a & b"), "a &amp; b");
    assert_eq!(xml_escape("<tag>"), "&lt;tag&gt;");
    assert_eq!(xml_escape("\"quoted\""), "&quot;quoted&quot;");
}
