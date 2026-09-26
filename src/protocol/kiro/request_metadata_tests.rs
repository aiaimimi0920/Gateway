//! Request metadata, machine identity and legacy packing contracts.
use super::*;
use crate::protocol::canonical::{CanonicalTool, ContentPart};

fn make_text_message(role: MessageRole, text: &str) -> CanonicalMessage {
    CanonicalMessage {
        role,
        content: vec![ContentPart::Text {
            text: text.to_string(),
        }],
        name: None,
        tool_call_id: None,
        tool_calls: Vec::new(),
    }
}

fn make_request(messages: Vec<CanonicalMessage>) -> CanonicalRelayRequest {
    CanonicalRelayRequest {
        protocol_family: crate::protocol::canonical::ProtocolFamily::OpenAi,
        endpoint_kind: crate::protocol::canonical::EndpointKind::ChatCompletions,
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
fn map_model_uses_kiro_claude_ids() {
    assert_eq!(map_model("claude-sonnet-4-20250514"), "claude-sonnet-4.5");
    assert_eq!(map_model("claude-sonnet-4.6"), "claude-sonnet-4.6");
    assert_eq!(map_model("claude-opus-4.5"), "claude-opus-4.5");
    assert_eq!(map_model("claude-haiku-4-20250514"), "claude-haiku-4.5");
}

#[test]
fn reserved_payload_extra_keys_skip_refresh_material() {
    assert!(is_reserved_payload_extra_key("kiroRefreshToken"));
    assert!(is_reserved_payload_extra_key("kiro-auth-region"));
    assert!(!is_reserved_payload_extra_key("profileArn"));
}

#[test]
fn generate_machine_id_normalizes_uuid_and_hashes_refresh_token() {
    let normalized =
        generate_machine_id(Some("2582956e-cc88-4669-b546-07adbffcb894"), None).unwrap();
    assert_eq!(normalized.len(), 64);
    let hashed = generate_machine_id(None, Some("refresh-token-123")).unwrap();
    assert_eq!(hashed.len(), 64);
}

#[test]
fn pack_kiro_builds_current_message_and_history() {
    let mut req = make_request(vec![
        make_text_message(MessageRole::System, "Be precise."),
        make_text_message(MessageRole::User, "Hello"),
        make_text_message(MessageRole::Assistant, "Hi"),
        make_text_message(MessageRole::User, "Read the file"),
    ]);
    req.tools.push(CanonicalTool {
        tool_type: "function".to_string(),
        name: Some("read_file".to_string()),
        description: Some("Read a file".to_string()),
        input_schema: Some(json!({"type":"object","properties":{"path":{"type":"string"}}})),
        raw: HashMap::new(),
    });

    let packed = pack_kiro(&req, "claude-sonnet-4-20250514").unwrap();
    assert_eq!(
        packed["conversationState"]["currentMessage"]["userInputMessage"]["content"],
        "Read the file"
    );
    assert_eq!(
        packed["conversationState"]["history"][0]["userInputMessage"]["content"],
        "Be precise."
    );
    assert_eq!(
        packed["conversationState"]["currentMessage"]["userInputMessage"]
            ["userInputMessageContext"]["tools"][0]["toolSpecification"]["name"],
        "read_file"
    );
}
