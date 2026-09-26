use super::{parse_raw_event, ParsedEvent};
use serde_json::json;

#[test]
fn provider_error_takes_precedence_over_stream_and_complete_payloads() {
    let events = parse_raw_event(&json!({
        "error_code": "5015",
        "error_message": "User not activated",
        "messageStart": {"model": "ignored"},
        "choices": [{"message": {"content": "ignored"}}]
    }));
    assert!(matches!(
        events.as_slice(),
        [ParsedEvent::ProviderError { code, message }]
            if code == "5015" && message == "User not activated"
    ));
}

#[test]
fn wrapped_event_presence_takes_precedence_over_typed_events() {
    let events = parse_raw_event(&json!({
        "messageStart": null,
        "type": "content_block_delta",
        "delta": {"type": "text_delta", "text": "ignored"},
        "model": "ignored"
    }));
    assert!(matches!(
        events.as_slice(),
        [ParsedEvent::Start {
            model: None,
            usage: None
        }]
    ));
}

#[test]
fn typed_delta_emits_text_once_before_tool_arguments() {
    let events = parse_raw_event(&json!({
        "type": "content_block_delta",
        "index": 4,
        "delta": {
            "type": "text_delta",
            "text": "selected",
            "delta": {"text": "ignored"},
            "toolUse": {"input": "{}"}
        }
    }));
    assert!(matches!(
        events.as_slice(),
        [ParsedEvent::Text(text), ParsedEvent::ToolDelta { index: 4, partial }]
            if text == "selected" && partial == "{}"
    ));
}

#[test]
fn unknown_string_type_is_consumed_without_complete_response_fallback() {
    let mut raw = json!({
        "type": "unknown",
        "model": "selected-model",
        "choices": [{"message": {"content": "selected-text"}}]
    });
    assert!(parse_raw_event(&raw).is_empty());
    raw["type"] = json!(42);
    assert!(matches!(
        parse_raw_event(&raw).as_slice(),
        [ParsedEvent::Start { model: Some(model), .. }, ParsedEvent::Text(text), ParsedEvent::Finish { .. }]
            if model == "selected-model" && text == "selected-text"
    ));
}
