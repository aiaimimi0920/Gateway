//! Trust-boundary tests do not infer new platform fields or accept unknown failure-shaped events.
use super::{
    machine::{Action, Machine, BODY_LIMIT, FRAME_LIMIT, TEXT_LIMIT},
    *,
};
use serde_json::{json, Value};
fn ready() -> Machine {
    let mut machine = Machine::new("init".into());
    assert!(matches!(machine.push(&json!({"session_id":"s","event":{"type":"session.created","client_event_id":"init"}}).to_string()).unwrap(),Action::None));
    assert!(matches!(
        machine
            .push(
                &json!({"event":{"type":"conversation.attached","conversation":{"id":"s"}}})
                    .to_string()
            )
            .unwrap(),
        Action::Turn(_)
    ));
    machine
}
fn event(kind: &str) -> Value {
    json!({"session_id":"s","event":{"type":kind}})
}
#[test]
fn grok_web_machine_never_resends_turn_and_rejects_mismatched_session_or_event() {
    for value in [
        json!({"session_id":"s","event":{"type":"session.created","client_event_id":"foreign"}}),
        json!({"session_id":"foreign","event":{"type":"response.output_text.delta","delta":"wrong"}}),
        json!({"event":{"type":"conversation.attached","conversation":{"id":"foreign"}}}),
    ] {
        assert!(ready().push(&value.to_string()).is_err());
    }
    let mut machine = ready();
    assert!(matches!(machine.push(&json!({"session_id":"s","event":{"type":"session.created","client_event_id":"init"}}).to_string()).unwrap(),Action::None));
    assert!(Machine::new("init".into())
        .push(&event("response.done").to_string())
        .is_err());
}
#[test]
fn grok_web_machine_unknown_response_and_changing_observed_response_ids_fail_closed() {
    for kind in ["response.failed", "response.incomplete", "response.unknown"] {
        assert!(ready().push(&event(kind).to_string()).is_err());
    }
    let mut machine = ready();
    machine.push(&json!({"event":{"type":"response.output_text.delta","response_id":"one","delta":"text"}}).to_string()).unwrap();
    assert!(machine
        .push(
            &json!({"event":{"type":"response.done","response":{"id":"two","status":"completed"}}})
                .to_string()
        )
        .is_err());
}
#[test]
fn grok_web_machine_analysis_is_not_answer_and_done_text_is_only_fallback() {
    let mut machine = ready();
    assert!(matches!(machine.push(&json!({"event":{"type":"response.chunk","chunk":{"text":{"text":"private reasoning","channel":"ANALYSIS"}}}}).to_string()).unwrap(),Action::None));
    assert!(machine
        .push(
            &json!({"event":{"type":"response.done","response":{"id":"r","status":"completed"}}})
                .to_string()
        )
        .is_err());
    let mut machine = ready();
    for (text, emitted) in [("answer", true), ("duplicate", false)] {
        assert_eq!(
            matches!(
                machine
                    .push(
                        &json!({"event":{"type":"response.output_text.done","text":text}})
                            .to_string()
                    )
                    .unwrap(),
                Action::Text(_)
            ),
            emitted
        );
    }
}
#[test]
fn grok_web_machine_frame_total_text_limits_and_media_tools_are_explicit_failures() {
    assert!(ready().push(&"x".repeat(FRAME_LIMIT + 1)).is_err());
    let mut machine = ready();
    let text = "a".repeat(TEXT_LIMIT / 4);
    for _ in 0..4 {
        machine
            .push(&json!({"event":{"type":"response.output_text.delta","delta":text}}).to_string())
            .unwrap();
    }
    assert!(machine
        .push(&json!({"event":{"type":"response.output_text.delta","delta":"x"}}).to_string())
        .is_err());
    let mut machine = ready();
    let padding = "p".repeat(FRAME_LIMIT - 128);
    for _ in 0..(BODY_LIMIT / FRAME_LIMIT) {
        machine
            .push(&json!({"event":{"type":"pong"},"padding":padding}).to_string())
            .unwrap();
    }
    assert!(machine
        .push(&json!({"event":{"type":"pong"},"padding":padding}).to_string())
        .is_err());
    for chunk in [json!({"tool_result":{}}), json!({"render_citation":{}})] {
        assert!(ready()
            .push(&json!({"event":{"type":"response.chunk","chunk":chunk}}).to_string())
            .is_err());
    }
    let initial = request::initial("fast", "init");
    assert_eq!(
        initial["event"]["session"]["x_grok"]["enable_image_generation"],
        false
    );
    assert_eq!(
        initial["event"]["session"]["x_grok"]["enable_side_by_side"],
        false
    );
}
