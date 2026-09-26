use super::{normalize_realtime_item, parse_openai_session_tools, RealtimeSession};
use crate::protocol::canonical::{ContentPart, EndpointKind, MessageRole, ProtocolFamily};
use serde_json::json;

#[test]
fn instructions_supply_a_user_turn_when_session_has_no_user_message() {
    let mut session = RealtimeSession::new("session-model".to_string());
    session.instructions = Some("Return a short answer".to_string());
    let request = session.build_request(None);

    assert!(matches!(
        request.protocol_family,
        ProtocolFamily::OpenAiRealtime
    ));
    assert!(matches!(
        request.endpoint_kind,
        EndpointKind::ChatCompletions
    ));
    assert!(request.stream);
    assert_eq!(request.requested_model.as_deref(), Some("session-model"));
    assert_eq!(request.messages.len(), 2);
    assert!(matches!(request.messages[0].role, MessageRole::System));
    assert!(matches!(request.messages[1].role, MessageRole::User));
    for message in &request.messages {
        assert!(matches!(
            &message.content[0],
            ContentPart::Text { text } if text == "Return a short answer"
        ));
    }
    assert!(session.messages.is_empty());
}

#[test]
fn response_overrides_preserve_session_state_and_existing_history() {
    let mut session = RealtimeSession::new("session-model".to_string());
    session.instructions = Some("Session instructions".to_string());
    session.messages.push(
        normalize_realtime_item(&json!({
            "role": "user", "content": [{"input_text": "Original question"}]
        }))
        .unwrap(),
    );
    let response = json!({
        "model": "response-model", "instructions": "Response instructions",
        "metadata": {"client": "synthetic"}
    });
    let request = session.build_request(Some(&response));

    assert_eq!(request.requested_model.as_deref(), Some("response-model"));
    assert_eq!(request.messages.len(), 2);
    assert!(matches!(
        &request.messages[0].content[0],
        ContentPart::Text { text } if text == "Response instructions"
    ));
    assert!(matches!(
        &request.messages[1].content[0],
        ContentPart::Text { text } if text == "Original question"
    ));
    assert_eq!(request.raw_body, response);
    assert_eq!(session.model, "session-model");
    assert_eq!(
        session.instructions.as_deref(),
        Some("Session instructions")
    );
    assert_eq!(session.messages.len(), 1);
}

#[test]
fn omitted_tool_overrides_inherit_and_explicit_empty_tools_clear() {
    let mut session = RealtimeSession::new("session-model".to_string());
    (session.tools, session.tool_choice) = parse_openai_session_tools(&json!({
        "tools": [{"type": "function", "name": "weather", "parameters": {"type": "object"}}],
        "tool_choice": "required"
    }))
    .unwrap();

    let inherited = session.build_request(Some(&json!({"instructions": "Call a tool"})));
    assert_eq!(inherited.tools.len(), 1);
    assert_eq!(inherited.tools[0].name.as_deref(), Some("weather"));
    assert_eq!(inherited.tool_choice, Some(json!("required")));

    let cleared = session.build_request(Some(&json!({"tools": [], "tool_choice": "none"})));
    assert!(cleared.tools.is_empty());
    assert_eq!(cleared.tool_choice, Some(json!("none")));
    assert_eq!(session.tools.len(), 1);
    assert_eq!(session.tool_choice, Some(json!("required")));
}

#[test]
fn conversation_items_preserve_text_order_and_structured_tool_outputs() {
    let message = normalize_realtime_item(&json!({
        "content": [{"text": "first"}, {"input_text": "second"}, {"image_url": "ignored"}]
    }))
    .unwrap();
    assert!(matches!(message.role, MessageRole::User));
    assert!(matches!(&message.content[0], ContentPart::Text { text } if text == "first\nsecond"));

    for output in [json!("{\"count\":2}"), json!({"count": 2})] {
        let message = normalize_realtime_item(&json!({
            "type": "function_call_output", "call_id": "call-client", "name": "weather", "output": output
        }))
        .unwrap();
        assert!(matches!(message.role, MessageRole::Tool));
        assert_eq!(message.tool_call_id.as_deref(), Some("call-client"));
        assert_eq!(message.name.as_deref(), Some("weather"));
        assert!(
            matches!(&message.content[0], ContentPart::Json { value } if value == &json!({"count": 2}))
        );
    }
    let plain =
        normalize_realtime_item(&json!({"type": "function_call_output", "output": "plain text"}))
            .unwrap();
    assert!(matches!(&plain.content[0], ContentPart::Text { text } if text == "plain text"));
    assert!(normalize_realtime_item(&json!({"type": "unsupported"})).is_none());
}
