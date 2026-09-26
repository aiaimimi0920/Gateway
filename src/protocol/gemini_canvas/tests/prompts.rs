use super::*;

#[test]
fn text_prompt_renders_system_and_user_transcript() {
    let req = CanonicalRelayRequest {
        protocol_family: ProtocolFamily::OpenAi,
        endpoint_kind: EndpointKind::ChatCompletions,
        requested_model: Some(GEMINI_CANVAS_DEFAULT_TEXT_MODEL.to_string()),
        stream: false,
        messages: vec![
            CanonicalMessage {
                role: MessageRole::System,
                content: vec![ContentPart::Text {
                    text: "You are terse.".to_string(),
                }],
                name: None,
                tool_call_id: None,
                tool_calls: vec![],
            },
            CanonicalMessage {
                role: MessageRole::User,
                content: vec![ContentPart::Text {
                    text: "Reply with exactly: ok".to_string(),
                }],
                name: None,
                tool_call_id: None,
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

    let prompt = prompt_for_text_request(&req, "missing", "missing_code").unwrap();
    assert!(prompt.contains("You are terse."));
    assert!(prompt.contains("User request:\nReply with exactly: ok"));
    assert!(prompt.contains("Return exactly this text and nothing else:\nok"));
}

#[test]
fn text_prompt_injects_tool_xml_for_required_tool_requests() {
    let req = CanonicalRelayRequest {
        protocol_family: ProtocolFamily::OpenAi,
        endpoint_kind: EndpointKind::ChatCompletions,
        requested_model: Some(GEMINI_CANVAS_DEFAULT_TEXT_MODEL.to_string()),
        stream: false,
        messages: vec![
            CanonicalMessage {
                role: MessageRole::System,
                content: vec![ContentPart::Text {
                    text: "You must call exactly one tool and do not answer directly.".to_string(),
                }],
                name: None,
                tool_call_id: None,
                tool_calls: vec![],
            },
            CanonicalMessage {
                role: MessageRole::User,
                content: vec![ContentPart::Text {
                    text: "Use only the weather tool for Hangzhou.".to_string(),
                }],
                name: None,
                tool_call_id: None,
                tool_calls: vec![],
            },
        ],
        tools: vec![CanonicalTool {
            tool_type: "function".to_string(),
            name: Some("weather".to_string()),
            description: Some("Return the current weather for a city.".to_string()),
            input_schema: Some(json!({
                "type": "object",
                "properties": { "city": { "type": "string" } },
                "required": ["city"],
            })),
            raw: HashMap::new(),
        }],
        tool_choice: Some(json!("required")),
        reasoning: None,
        metadata: None,
        raw_body: json!({}),
        previous_response_id: None,
        explicit_session_key: None,
        extra: HashMap::new(),
    };

    let prompt = prompt_for_text_request(&req, "missing", "missing_code").unwrap();
    assert!(prompt.contains("TOOL CALL FORMAT"));
    assert!(prompt.contains("<tool_calls>"));
    assert!(prompt.contains("weather"));
    assert!(prompt.contains("Use only the weather tool for Hangzhou."));
}

#[test]
fn text_prompt_serializes_tool_history_for_roundtrip() {
    let req = CanonicalRelayRequest {
        protocol_family: ProtocolFamily::OpenAi,
        endpoint_kind: EndpointKind::ChatCompletions,
        requested_model: Some(GEMINI_CANVAS_DEFAULT_TEXT_MODEL.to_string()),
        stream: false,
        messages: vec![
            CanonicalMessage {
                role: MessageRole::User,
                content: vec![ContentPart::Text {
                    text: "Use the weather tool for Hangzhou.".to_string(),
                }],
                name: None,
                tool_call_id: None,
                tool_calls: vec![],
            },
            CanonicalMessage {
                role: MessageRole::Assistant,
                content: vec![ContentPart::Text {
                    text: String::new(),
                }],
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
                content: vec![ContentPart::Text {
                    text: "{\"city\":\"Hangzhou\",\"condition\":\"sunny\"}".to_string(),
                }],
                name: None,
                tool_call_id: Some("call_weather".to_string()),
                tool_calls: vec![],
            },
            CanonicalMessage {
                role: MessageRole::User,
                content: vec![ContentPart::Text {
                    text: "Produce the final answer now.".to_string(),
                }],
                name: None,
                tool_call_id: None,
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

    let prompt = prompt_for_text_request(&req, "missing", "missing_code").unwrap();
    assert!(prompt.contains("Caller-provided result for `weather`"));
    assert!(prompt.contains("Produce the final answer now."));
}

#[test]
fn text_prompt_surfaces_exact_answer_requirement_for_tool_history() {
    let req = CanonicalRelayRequest {
        protocol_family: ProtocolFamily::OpenAi,
        endpoint_kind: EndpointKind::ChatCompletions,
        requested_model: Some(GEMINI_CANVAS_DEFAULT_TEXT_MODEL.to_string()),
        stream: false,
        messages: vec![
            CanonicalMessage {
                role: MessageRole::System,
                content: vec![ContentPart::Text {
                    text: "You are terse. Reply with exactly: gemini roundtrip ok".to_string(),
                }],
                name: None,
                tool_call_id: None,
                tool_calls: vec![],
            },
            CanonicalMessage {
                role: MessageRole::Assistant,
                content: vec![],
                name: None,
                tool_call_id: None,
                tool_calls: vec![crate::protocol::canonical::CanonicalToolCall {
                    id: Some("call_weather".to_string()),
                    call_type: "function".to_string(),
                    name: Some("weather".to_string()),
                    arguments: Some("{\"city\":\"Hangzhou\"}".to_string()),
                    raw: HashMap::new(),
                }],
            },
            CanonicalMessage {
                role: MessageRole::Tool,
                content: vec![ContentPart::Text {
                    text: "{\"city\":\"Hangzhou\",\"condition\":\"sunny\"}".to_string(),
                }],
                name: None,
                tool_call_id: Some("call_weather".to_string()),
                tool_calls: vec![],
            },
            CanonicalMessage {
                role: MessageRole::User,
                content: vec![ContentPart::Text {
                    text: "Produce the final answer now.".to_string(),
                }],
                name: None,
                tool_call_id: None,
                tool_calls: vec![],
            },
        ],
        tools: vec![],
        tool_choice: None,
        reasoning: None,
        metadata: Some(HashMap::new()),
        raw_body: json!({}),
        previous_response_id: None,
        explicit_session_key: None,
        extra: HashMap::new(),
    };

    let prompt = prompt_for_text_request(&req, "missing", "missing_code").unwrap();
    assert!(prompt.contains("Required exact final answer:\ngemini roundtrip ok"));
    assert!(prompt.contains("Return exactly that text and nothing else."));
}

#[test]
fn text_prompt_preserves_system_instruction_for_plain_text_requests() {
    let req = CanonicalRelayRequest {
        protocol_family: ProtocolFamily::OpenAi,
        endpoint_kind: EndpointKind::ChatCompletions,
        requested_model: Some(GEMINI_CANVAS_DEFAULT_TEXT_MODEL.to_string()),
        stream: false,
        messages: vec![
            CanonicalMessage {
                role: MessageRole::System,
                content: vec![ContentPart::Text {
                    text: "You are terse.".to_string(),
                }],
                name: None,
                tool_call_id: None,
                tool_calls: vec![],
            },
            CanonicalMessage {
                role: MessageRole::User,
                content: vec![ContentPart::Text {
                    text: "Reply with exactly: plain text ok".to_string(),
                }],
                name: None,
                tool_call_id: None,
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

    let prompt = prompt_for_text_request(&req, "missing", "missing_code").unwrap();
    assert!(prompt.contains("You are terse."));
    assert!(prompt.contains("User request:\nReply with exactly: plain text ok"));
    assert!(prompt.contains("Return exactly this text and nothing else:\nplain text ok"));
}
