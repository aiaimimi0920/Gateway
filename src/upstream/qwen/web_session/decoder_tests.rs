//! Incremental framing, output fidelity, identity, explicit terminal and resource limits.
use super::events::{Action, Decoder};
use bytes::Bytes;
use serde_json::json;
fn decoder() -> Decoder {
    Decoder::new("chat".into())
}
fn feed(d: &mut Decoder, value: serde_json::Value) {
    d.feed(Bytes::from(format!("data: {value}\n\n"))).unwrap();
}
#[test]
fn qwen_web_current_decoder_byte_split_crlf_and_final_delta_preserve_text() {
    let wire = ":keepalive\r\nevent: message\r\nid: 1\r\ndata: {\"choices\":[{\"delta\":{\"content\":\" 原文中文 \",\"phase\":\"answer\"}}],\"done\":true}\r\n\r\n";
    let mut d = decoder();
    let mut text = String::new();
    let mut done = 0;
    for b in wire.as_bytes() {
        d.feed(Bytes::from(vec![*b])).unwrap();
        while let Some(action) = d.next().unwrap() {
            match action {
                Action::Text(s) => text.push_str(&s),
                Action::Done => {
                    done += 1;
                    break;
                }
            }
        }
    }
    assert_eq!(text, " 原文中文 ");
    assert_eq!(done, 1);
    assert!(d.usage.is_none());
}
#[test]
fn qwen_web_current_decoder_finished_phase_is_not_terminal_and_usage_is_not_invented() {
    let mut d = decoder();
    feed(
        &mut d,
        json!({"choices":[{"delta":{"content":"answer","phase":"answer","status":"finished"}}],"usage":{"input_tokens":2}}),
    );
    assert!(matches!(d.next().unwrap(), Some(Action::Text(_))));
    assert!(d.next().unwrap().is_none());
    assert!(d.usage.is_none());
    feed(
        &mut d,
        json!({"usage":{"input_tokens":2,"output_tokens":3}}),
    );
    assert!(d.next().unwrap().is_none());
    assert_eq!(d.usage.as_ref().unwrap().total_tokens, 5);
    feed(&mut d, json!({"usage":{"input_tokens":2},"done":true}));
    assert!(matches!(d.next().unwrap(), Some(Action::Done)));
    assert!(d.usage.is_none());
}
#[test]
fn qwen_web_current_decoder_rejects_error_stopped_ambiguous_usage_and_identity() {
    for value in [
        json!({"error":"synthetic-private","done":true}),
        json!({"response.stopped":{}}),
        json!({"usage":{"input_tokens":2,"prompt_tokens":3,"output_tokens":1}}),
        json!({"response.created":{"chat_id":"foreign"}}),
        json!({"done":"true"}),
        json!({"unknown":"private"}),
    ] {
        let mut d = decoder();
        feed(&mut d, value);
        let error = d.next().err().unwrap();
        assert!(!error.to_string().contains("synthetic-private"));
    }
    let mut d = decoder();
    feed(
        &mut d,
        json!({"response.created":{"chat_id":"chat","response_id":"a"}}),
    );
    assert!(d.next().unwrap().is_none());
    feed(&mut d, json!({"response_id":"b","done":true}));
    assert!(d.next().is_err());
}
#[test]
fn qwen_web_current_decoder_rejects_unrepresentable_output_and_malformed_frames() {
    for delta in [
        json!({"content":"a","audio":{}}),
        json!({"role":"tool","content":"a"}),
        json!({"phase":2,"content":"a"}),
        json!({"content":[{"text":"a"}]}),
        json!({"function_call":{"name":"tool"}}),
        json!({"extra":{"web_search_info":[]}}),
    ] {
        let mut d = decoder();
        feed(&mut d, json!({"choices":[{"delta":delta}]}));
        assert!(d.next().is_err());
    }
    for raw in [
        b"data: {bad}\n\n".as_slice(),
        b"data: \xff\n\n",
        b"data: []\n\n",
        b"unknown: field\n\n",
    ] {
        let mut d = decoder();
        d.feed(Bytes::copy_from_slice(raw)).unwrap();
        assert!(d.next().is_err());
    }
}
#[test]
fn qwen_web_current_decoder_enforces_wire_frame_event_and_text_limits() {
    let mut d = decoder();
    assert!(d
        .feed(Bytes::from(vec![b'x'; 8 * 1024 * 1024 + 1]))
        .is_err());
    let mut d = decoder();
    d.feed(Bytes::from(vec![b'x'; 1024 * 1024 + 1])).unwrap();
    assert!(d.next().is_err());
    let mut d = decoder();
    for _ in 0..16384 {
        feed(&mut d, json!({"done":false}));
        assert!(d.next().unwrap().is_none());
    }
    feed(&mut d, json!({"done":false}));
    assert!(d.next().is_err());
    let mut d = decoder();
    for _ in 0..2 {
        feed(
            &mut d,
            json!({"choices":[{"delta":{"content":"x".repeat(800_000)}}]}),
        );
        assert!(matches!(d.next().unwrap(), Some(Action::Text(_))));
    }
    feed(
        &mut d,
        json!({"choices":[{"delta":{"content":"x".repeat(800_000)}}]}),
    );
    assert!(d.next().is_err());
}
