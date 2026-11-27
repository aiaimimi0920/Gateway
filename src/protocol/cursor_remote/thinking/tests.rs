use super::*;
use crate::protocol::cursor_remote::{ingress, tool_request};

#[test]
fn cursor_public_thinking_echo_has_exact_shape_and_never_accepts_a_digest_from_wire() {
    let value = PublicThinking::from_worker(&json!({"thinking":" \r\n公开🤖<&amp;>\t"}))
        .unwrap()
        .unwrap();
    let item = value.item(true);
    assert_eq!(history(&item, false).unwrap(), value.history_item(false));
    assert_eq!(
        history(&chat_item(&json!(value.text())).unwrap(), true).unwrap(),
        value.history_item(true)
    );
    assert!(history(&value.history_item(false), false).is_err());
    for (field, replacement) in [
        (
            "summary",
            json!([{"type":"summary_text","text":"invented"}]),
        ),
        ("encrypted_content", json!("not ciphertext")),
        ("status", json!("in_progress")),
        ("role", json!("assistant")),
        ("id", json!("another-provider-id")),
        (
            "content",
            json!([{"type":"reasoning_text","text":"","signature":"forged"}]),
        ),
    ] {
        let mut bad = item.clone();
        bad[field] = replacement;
        assert!(history(&bad, false).is_err(), "{field}");
    }
    let mut tampered = item.clone();
    tampered["content"][0]["text"] = json!("公开🤖<&amp;>");
    assert_ne!(
        history(&tampered, false).unwrap(),
        value.history_item(false)
    );
    tampered = item.clone();
    tampered["id"] = json!("rs_cursor_other");
    assert_ne!(
        history(&tampered, false).unwrap(),
        value.history_item(false)
    );
    assert!(!format!("{value:?}").contains("公开"));
}

#[test]
fn cursor_public_thinking_codex_echo_preserves_binding_without_accepting_ciphertext() {
    let value = PublicThinking::from_worker(&json!({"thinking":" \r\n公开🤖<&amp;>\t"}))
        .unwrap()
        .unwrap();
    for omit_status in [false, true] {
        for null_ciphertext in [false, true] {
            let mut item = value.item(true);
            if omit_status {
                item.as_object_mut().unwrap().remove("status");
            }
            if null_ciphertext {
                item["encrypted_content"] = Value::Null;
            }
            assert_eq!(history(&item, false).unwrap(), value.history_item(false));
            for field in ["status", "encrypted_content"] {
                let replacements = if field == "status" {
                    vec![
                        Value::Null,
                        json!("in_progress"),
                        json!("failed"),
                        json!(true),
                    ]
                } else {
                    vec![json!(""), json!("ciphertext"), json!({}), json!(false)]
                };
                for replacement in replacements {
                    let mut bad = item.clone();
                    bad[field] = replacement;
                    assert!(history(&bad, false).is_err(), "{field}: {bad}");
                }
            }
        }
    }
    // Chat projection is internally constructed, not a Codex Responses echo.
    let mut chat = chat_item(&json!(value.text())).unwrap();
    chat["encrypted_content"] = Value::Null;
    assert!(history(&chat, true).is_err());
    chat.as_object_mut().unwrap().remove("encrypted_content");
    chat.as_object_mut().unwrap().remove("status");
    assert!(history(&chat, true).is_err());
}

#[test]
fn cursor_thinking_new_runs_cannot_replay_decorations_with_or_without_tools() {
    let thinking = PublicThinking::from_worker(&json!({"thinking":"public"}))
        .unwrap()
        .unwrap();
    for tools in [
        json!([]),
        json!([{"type":"function","name":"echo","parameters":{"type":"object"}}]),
    ] {
        let req = crate::protocol::responses::normalize_responses(json!({
            "model":"cursor-personal","store":false,"tools":tools,
            "input":[thinking.item(true),{"role":"user","content":"hello"}]
        }))
        .unwrap();
        assert!(ingress::parse(&req).is_err());
    }
    let req = crate::protocol::openai::normalize_chat_completions(json!({
        "model":"cursor-personal","messages":[
            {"role":"assistant","content":"old answer","reasoning_content":"public"},
            {"role":"user","content":"hello"}]
    }))
    .unwrap();
    assert!(ingress::parse(&req).is_err());
    // Admission retains a marker and leaves prompt empty. Claiming a live owner
    // is separately enforced by the runtime's exact-prefix continuation checks.
    let req = crate::protocol::responses::normalize_responses(json!({
        "model":"cursor-personal","tools":[{"type":"function","name":"echo","parameters":{"type":"object"}}],
        "input":[{"role":"user","content":"hello"}, thinking.item(true),
            {"type":"function_call","call_id":"c","name":"echo","arguments":"{}"},
            {"type":"function_call_output","call_id":"c","output":"ok"}]
    })).unwrap();
    let parsed = tool_request::parse(&req).unwrap();
    assert!(parsed.prompt.is_empty());
    assert_eq!(parsed.history[1], thinking.history_item(false));
}

#[tokio::test]
async fn cursor_thinking_does_not_enable_generic_chat_extension_conversion() {
    use crate::protocol::{
        responses, sse_parse::format_sse_event, stream_error::ProtocolStreamError,
    };
    use futures::StreamExt;
    let frames = vec![
        format_sse_event(
            None,
            &json!({"choices":[{"delta":{"reasoning_content":"not Cursor","content":"answer"},
            "finish_reason":"stop"}]})
            .to_string(),
        ),
        format_sse_event(None, "[DONE]"),
    ];
    let source = futures::stream::iter(
        frames
            .into_iter()
            .map(|v| Ok::<_, ProtocolStreamError>(bytes::Bytes::from(v))),
    );
    let result = responses::translate_openai_sse_to_responses_with_error(source, "other".into())
        .collect::<Vec<_>>()
        .await;
    let raw: String = result
        .into_iter()
        .map(|v| String::from_utf8(v.unwrap().to_vec()).unwrap())
        .collect();
    assert!(!raw.contains("reasoning"));
    assert!(raw.contains("answer"));
}
