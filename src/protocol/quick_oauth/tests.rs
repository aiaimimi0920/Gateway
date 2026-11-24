//! Focused unit tests for Amazon Quick EventStream framing, prompt/tool bridging, and OAuth validation.

use super::{
    context::{commercial_region, AuthorizationContext},
    eventstream::{decode_inbound_binary, encode_outbound_event, encode_test_eventstream_frame},
    messages::{extract_client_tool_calls, prepare_turn},
    DEFAULT_WS_HOST,
};
use crate::protocol::canonical::{
    CanonicalMessage, CanonicalRelayRequest, CanonicalTool, ContentPart, EndpointKind, MessageRole,
    ProtocolFamily,
};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde_json::{json, Value};
use std::collections::HashMap;

#[test]
fn quick_oauth_eventstream_roundtrip_and_crc_verification() {
    let inner = json!({
        "type": "text_delta",
        "payload": { "delta": "Hello Quick!" }
    });
    let outer = json!({
        "payload": STANDARD.encode(serde_json::to_vec(&inner).unwrap())
    });
    let frame = encode_test_eventstream_frame(
        &[
            (":message-type", "event"),
            (":event-type", "rawOutputEvent"),
        ],
        &serde_json::to_vec(&outer).unwrap(),
    );
    let decoded = decode_inbound_binary(&frame).expect("valid frame");
    assert_eq!(decoded.len(), 1);
    assert_eq!(decoded[0]["type"], "text_delta");
    assert_eq!(decoded[0]["payload"]["delta"], "Hello Quick!");

    let mut corrupted = frame.clone();
    let last = corrupted.len() - 1;
    corrupted[last] ^= 0x55;
    let err = decode_inbound_binary(&corrupted).expect_err("corrupted CRC must fail");
    assert_eq!(
        err.code.as_deref(),
        Some("quick_oauth_eventstream_crc_mismatch")
    );
}

#[test]
fn quick_oauth_outbound_raw_input_event_wraps_base64_json() {
    let encoded = encode_outbound_event("session_create", json!({ "session_id": "s1" })).unwrap();
    let outer: Value = serde_json::from_str(&encoded).unwrap();
    assert_eq!(outer["eventType"], "rawInputEvent");
    let raw_b64 = outer["payload"]["payload"].as_str().unwrap();
    let inner: Value = serde_json::from_slice(&STANDARD.decode(raw_b64).unwrap()).unwrap();
    assert_eq!(inner["type"], "session_create");
    assert_eq!(inner["payload"]["session_id"], "s1");
}

#[test]
fn quick_oauth_prepare_turn_builds_persona_override_and_tool_bridge() {
    let req = CanonicalRelayRequest {
        protocol_family: ProtocolFamily::OpenAi,
        endpoint_kind: EndpointKind::ChatCompletions,
        requested_model: Some("smart".to_owned()),
        stream: false,
        messages: vec![
            CanonicalMessage {
                role: MessageRole::System,
                content: vec![ContentPart::Text {
                    text: "Reply in concise JSON.".to_owned(),
                }],
                name: None,
                tool_call_id: None,
                tool_calls: Vec::new(),
            },
            CanonicalMessage {
                role: MessageRole::User,
                content: vec![ContentPart::Text {
                    text: "Check weather in Tokyo.".to_owned(),
                }],
                name: None,
                tool_call_id: None,
                tool_calls: Vec::new(),
            },
        ],
        tools: vec![CanonicalTool {
            tool_type: "function".to_owned(),
            name: Some("get_weather".to_owned()),
            description: Some("Lookup city weather".to_owned()),
            input_schema: Some(
                json!({"type": "object", "properties": {"city": {"type": "string"}}}),
            ),
            raw: HashMap::new(),
        }],
        tool_choice: Some(json!("auto")),
        reasoning: Some(json!({"effort": "high"})),
        metadata: None,
        raw_body: json!({}),
        previous_response_id: None,
        explicit_session_key: None,
        extra: HashMap::new(),
    };
    let turn = prepare_turn(&req, "smart").expect("valid turn");
    assert_eq!(turn.model, "smart");
    assert_eq!(turn.prompt, "Check weather in Tokyo.");
    assert_eq!(turn.thinking_effort.as_deref(), Some("high"));
    assert!(turn.has_client_tools);
    assert!(turn.system_prompt.contains("Reply in concise JSON."));
    assert!(turn.system_prompt.contains("Runtime directive:"));
    assert!(turn.system_prompt.contains("<client_tool_protocol>"));
    assert!(turn.system_prompt.contains("get_weather"));

    let assistant_raw =
        "Checking now.\n<client_tool_call>{\"name\":\"get_weather\",\"arguments\":{\"city\":\"Tokyo\"}}</client_tool_call>";
    let (visible, calls) = extract_client_tool_calls(assistant_raw);
    assert_eq!(visible, "Checking now.");
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].name.as_deref(), Some("get_weather"));
    assert_eq!(calls[0].arguments.as_deref(), Some("{\"city\":\"Tokyo\"}"));
}

#[test]
fn quick_oauth_context_validates_commercial_region_and_redacts_debug() {
    assert!(commercial_region("us-east-1"));
    assert!(!commercial_region("cn-north-1"));
    let ctx = AuthorizationContext {
        idc_region: "us-east-1".to_owned(),
        issuer_url: "https://d-1234567890.awsapps.com/start".to_owned(),
        tenant_url: None,
        ws_host: DEFAULT_WS_HOST.to_owned(),
        client_id: "client-id-1".to_owned(),
        client_secret: Some("super-secret-value".to_owned()),
        client_secret_expires_at: Some(2_000_000_000),
        actor_arn: Some("arn:aws:sts::123456789012:assumed-role/User".to_owned()),
        aws_account_id: Some("123456789012".to_owned()),
        email: Some("dev@example.com".to_owned()),
        display_name: Some("Dev User".to_owned()),
        account_name: None,
    };
    ctx.validate().expect("valid context");
    let dbg = format!("{ctx:?}");
    assert!(!dbg.contains("super-secret-value"));
    assert!(dbg.contains("<redacted>"));
}
