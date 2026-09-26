use super::normalize_messages;
use crate::protocol::canonical::{ContentPart, MessageRole};
use serde_json::json;

#[test]
fn preserves_system_and_interleaved_tool_result_order() {
    let request = normalize_messages(json!({
        "model": "claude-test",
        "system": [
            {"type": "text", "text": "first"},
            {"type": "thinking", "thinking": "ignored"},
            {"type": "text", "text": "second"}
        ],
        "messages": [{
            "role": "user",
            "content": [
                {"type": "text", "text": "before"},
                {"type": "tool_result", "tool_use_id": "toolu_1", "content": "result"},
                {"type": "text", "text": "after"}
            ]
        }]
    }))
    .unwrap();

    assert_eq!(
        request
            .messages
            .iter()
            .map(|message| message.role)
            .collect::<Vec<_>>(),
        vec![
            MessageRole::System,
            MessageRole::User,
            MessageRole::Tool,
            MessageRole::User,
        ]
    );
    assert_eq!(request.messages[0].text_content(), "first\nsecond");
    assert_eq!(request.messages[1].text_content(), "before");
    assert_eq!(request.messages[2].text_content(), "result");
    assert_eq!(request.messages[2].tool_call_id.as_deref(), Some("toolu_1"));
    assert_eq!(request.messages[3].text_content(), "after");
}

#[test]
fn preserves_assistant_tool_call_order_and_arguments() {
    let request = normalize_messages(json!({
        "messages": [{
            "role": "assistant",
            "content": [
                {"type": "tool_use", "id": "toolu_1", "name": "search", "input": {"q": "rust"}},
                {"type": "text", "text": "between"},
                {"type": "tool_use", "id": "toolu_2", "name": "sum", "input": {"a": 1, "b": 2}}
            ]
        }]
    }))
    .unwrap();

    let message = &request.messages[0];
    assert_eq!(message.role, MessageRole::Assistant);
    assert_eq!(message.text_content(), "between");
    assert_eq!(message.tool_calls.len(), 2);
    assert_eq!(message.tool_calls[0].id.as_deref(), Some("toolu_1"));
    assert_eq!(message.tool_calls[1].id.as_deref(), Some("toolu_2"));
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(
            message.tool_calls[0].arguments.as_deref().unwrap()
        )
        .unwrap(),
        json!({"q": "rust"})
    );
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(
            message.tool_calls[1].arguments.as_deref().unwrap()
        )
        .unwrap(),
        json!({"a": 1, "b": 2})
    );
}

#[test]
fn preserves_tools_and_tool_choice_normalization() {
    let request = normalize_messages(json!({
        "messages": [],
        "tools": [{
            "name": "weather",
            "description": "Weather lookup",
            "input_schema": {"type": "object", "required": ["city"]},
            "cache_control": {"type": "ephemeral"}
        }],
        "tool_choice": {"type": "tool", "name": "weather"}
    }))
    .unwrap();

    assert_eq!(request.tools.len(), 1);
    assert_eq!(request.tools[0].tool_type, "function");
    assert_eq!(request.tools[0].name.as_deref(), Some("weather"));
    assert_eq!(
        request.tools[0].description.as_deref(),
        Some("Weather lookup")
    );
    assert_eq!(
        request.tools[0].input_schema,
        Some(json!({"type": "object", "required": ["city"]}))
    );
    assert_eq!(
        request.tools[0].raw.get("cache_control"),
        Some(&json!({"type": "ephemeral"}))
    );
    assert_eq!(
        request.tool_choice,
        Some(json!({"type": "function", "function": {"name": "weather"}}))
    );

    let cases = [
        (json!("none"), Some(json!("none"))),
        (json!("required"), Some(json!("required"))),
        (json!("any"), Some(json!("required"))),
        (json!("unknown"), Some(json!("auto"))),
        (json!({"type": "none"}), Some(json!("none"))),
        (json!({"type": "any"}), Some(json!("required"))),
        (json!({"type": "tool"}), None),
        (json!(7), Some(json!(7))),
    ];
    for (input, expected) in cases {
        let normalized = normalize_messages(json!({
            "messages": [],
            "tool_choice": input
        }))
        .unwrap();
        assert_eq!(normalized.tool_choice, expected);
    }
}

#[test]
fn preserves_current_image_url_extraction() {
    let request = normalize_messages(json!({
        "messages": [{
            "role": "user",
            "content": [
                {"type": "image", "source": {"type": "url", "url": "https://example.test/a.png"}},
                {"type": "image", "source": {"type": "base64", "media_type": "image/png", "data": "AA=="}}
            ]
        }]
    }))
    .unwrap();

    assert!(matches!(
        &request.messages[0].content[0],
        ContentPart::ImageUrl { image_url, detail: None }
            if image_url == "https://example.test/a.png"
    ));
    assert!(matches!(
        &request.messages[0].content[1],
        ContentPart::ImageUrl { image_url, detail: None } if image_url.is_empty()
    ));
}

#[test]
fn preserves_invalid_message_errors() {
    let cases = [
        (json!({}), "missing or invalid `messages` array"),
        (
            json!({"messages": "not-an-array"}),
            "missing or invalid `messages` array",
        ),
        (
            json!({"messages": [{"content": "missing role"}]}),
            "message missing `role` field",
        ),
        (
            json!({"messages": [{"role": "system", "content": "wrong location"}]}),
            "unexpected Anthropic message role: system",
        ),
    ];

    for (body, expected_message) in cases {
        let error = normalize_messages(body).unwrap_err();
        assert_eq!(error.http_status, Some(400));
        assert_eq!(error.message, expected_message);
    }
}
