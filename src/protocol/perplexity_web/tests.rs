use super::*;
use crate::protocol::canonical::{
    CanonicalMessage, CanonicalRelayRequest, ContentPart, EndpointKind, MessageRole, ProtocolFamily,
};
use bytes::Bytes;
use serde_json::{json, Value};
use std::collections::HashMap;

pub(super) fn request() -> CanonicalRelayRequest {
    CanonicalRelayRequest {
        protocol_family: ProtocolFamily::OpenAi,
        endpoint_kind: EndpointKind::ChatCompletions,
        requested_model: Some("turbo".into()),
        stream: false,
        messages: vec![CanonicalMessage {
            role: MessageRole::User,
            content: vec![ContentPart::Text {
                text: "你好".into(),
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

fn event(value: Value) -> String {
    format!("data: {value}\r\n\r\n")
}
fn snapshot(text: &str) -> Value {
    json!({"blocks":[{"intended_usage":"ask_text","markdown_block":{"answer":text}}]})
}

#[test]
fn perplexity_web_single_turn_matches_the_pinned_auto_profile() {
    let body = pack_single_turn(&request()).unwrap();
    assert_eq!(body["query_str"], "你好");
    assert_eq!(body["params"]["model_preference"], "turbo");
    assert_eq!(body["params"]["mode"], "concise");
    assert_eq!(body["params"]["version"], "2.18");
    assert_eq!(body["params"]["sources"], json!(["web"]));
    assert_eq!(body["params"]["attachments"], json!([]));
    assert!(body["params"]["last_backend_uuid"].is_null());
    let context = body["params"]["frontend_context_uuid"].as_str().unwrap();
    let frontend = body["params"]["frontend_uuid"].as_str().unwrap();
    assert_ne!(context, frontend);
    assert_eq!(uuid::Uuid::parse_str(context).unwrap().get_version_num(), 4);
}

#[test]
fn perplexity_web_snapshots_replace_instead_of_append_and_need_terminal() {
    let mut decoder = Decoder::new();
    assert!(decoder
        .push(Bytes::from(event(snapshot("A"))))
        .unwrap()
        .is_none());
    assert!(decoder
        .push(Bytes::from(event(snapshot("AB"))))
        .unwrap()
        .is_none());
    let answer = decoder
        .push(Bytes::from_static(b"event: end_of_stream\n\n"))
        .unwrap()
        .unwrap();
    assert_eq!(answer.text, "AB");
    assert!(decoder.finish().is_ok());
}

#[test]
fn perplexity_web_legacy_final_is_nested_json_not_arbitrary_text() {
    let steps = json!([{"step_type":"FINAL","content":{"answer":json!({"answer":"完成","chunks":[]}).to_string()}}]);
    let raw = json!({"text":steps.to_string(),"read_write_token":"private-do-not-return"});
    let mut decoder = Decoder::new();
    let wire = format!("{}event: end_of_stream\r\n\r\n", event(raw));
    let answer = decoder.push(Bytes::from(wire)).unwrap().unwrap();
    assert_eq!(answer.text, "完成");
    assert!(!format!("{answer:?}").contains("private-do-not-return"));
}
