//! Strict CLI runtime products reject ambiguous wire without changing the default REST decoder.
use super::{event_stream::EventStreamParser, event_stream::KiroEvent};
use crate::upstream::client::kiro_api_key_test_support as wire;
use bytes::Bytes;
use serde_json::json;

fn consume(
    parser: &mut EventStreamParser,
    bytes: Vec<u8>,
) -> Result<(), crate::error::GatewayError> {
    parser.push(Bytes::from(bytes))?;
    while parser.next_event()?.is_some() {}
    parser.finish()
}
fn event(kind: &str, raw: &[u8]) -> Vec<u8> {
    wire::frame(&[(":message-type", "event"), (":event-type", kind)], raw)
}

#[test]
fn runtime_integrity_rejects_duplicate_critical_json_fields_without_changing_legacy() {
    for (kind, raw) in [
        (
            "assistantResponseEvent",
            r#"{"content":"first","content":"second"}"#,
        ),
        (
            "assistantResponseEvent",
            r#"{"content":"same","con\u0074ent":"same"}"#,
        ),
        (
            "toolUseEvent",
            r#"{"name":"read","toolUseId":"a","toolUseId":"b","stop":true}"#,
        ),
        (
            "toolUseEvent",
            r#"{"name":"read","name":"other","toolUseId":"a","stop":true}"#,
        ),
        (
            "toolUseEvent",
            r#"{"name":"read","toolUseId":"a","input":"{}","input":"{}","stop":true}"#,
        ),
        (
            "toolUseEvent",
            r#"{"name":"read","toolUseId":"a","stop":false,"stop":true}"#,
        ),
        (
            "contextUsageEvent",
            r#"{"contextUsagePercentage":1,"contextUsagePercentage":2}"#,
        ),
    ] {
        let bytes = [wire::event_frame("visible"), event(kind, raw.as_bytes())].concat();
        assert!(
            consume(&mut EventStreamParser::for_api_key(), bytes.clone()).is_err(),
            "{raw}"
        );
        consume(&mut EventStreamParser::default(), bytes).unwrap();
    }
}

#[test]
fn runtime_integrity_known_payloads_require_objects_without_changing_legacy() {
    for (kind, raw) in [
        ("assistantResponseEvent", r#"["text"]"#),
        ("toolUseEvent", r#"["a","read","{}",true]"#),
        ("contextUsageEvent", "[1]"),
    ] {
        let bytes = [wire::event_frame("visible"), event(kind, raw.as_bytes())].concat();
        assert!(consume(&mut EventStreamParser::for_api_key(), bytes.clone()).is_err());
        consume(&mut EventStreamParser::default(), bytes).unwrap();
    }
    consume(
        &mut EventStreamParser::for_api_key(),
        event(
            "assistantResponseEvent",
            b" \r\n\t{\"content\":\"visible\"}",
        ),
    )
    .unwrap();
}

#[test]
fn runtime_integrity_rejects_ambiguous_event_headers_without_changing_legacy() {
    for fields in [
        vec![
            (":message-type", "event"),
            (":message-type", "event"),
            (":event-type", "assistantResponseEvent"),
        ],
        vec![
            (":message-type", "event"),
            (":event-type", "ignored"),
            (":event-type", "assistantResponseEvent"),
        ],
        vec![(":event-type", "assistantResponseEvent")],
        vec![
            (":message-type", "unknown"),
            (":event-type", "assistantResponseEvent"),
        ],
        vec![(":message-type", "event")],
    ] {
        let bytes = [
            wire::event_frame("visible"),
            wire::frame(&fields, br#"{"content":"later"}"#),
        ]
        .concat();
        assert!(
            consume(&mut EventStreamParser::for_api_key(), bytes.clone()).is_err(),
            "{fields:?}"
        );
        consume(&mut EventStreamParser::default(), bytes).unwrap();
    }
}

#[test]
fn runtime_integrity_rejects_invalid_utf8_headers_but_keeps_unknown_events() {
    for marker in [b"trace".as_slice(), b"valid".as_slice()] {
        let mut invalid = wire::frame(
            &[
                (":message-type", "event"),
                (":event-type", "ignored"),
                ("trace", "valid"),
            ],
            b"not-json",
        );
        let index = invalid
            .windows(marker.len())
            .position(|w| w == marker)
            .unwrap();
        invalid[index] = 0xff;
        let end = invalid.len() - 4;
        let crc = crc::Crc::<u32>::new(&crc::CRC_32_ISO_HDLC).checksum(&invalid[..end]);
        invalid[end..].copy_from_slice(&crc.to_be_bytes());
        let bytes = [wire::event_frame("visible"), invalid].concat();
        assert!(consume(&mut EventStreamParser::for_api_key(), bytes.clone()).is_err());
        consume(&mut EventStreamParser::default(), bytes).unwrap();
    }
    consume(
        &mut EventStreamParser::for_api_key(),
        [
            wire::event_frame("visible"),
            event("futureMetadata", b"not-json"),
        ]
        .concat(),
    )
    .unwrap();
}

#[test]
fn runtime_integrity_optional_fields_reject_null_and_nonstring_message_type() {
    for field in ["name", "input", "stop"] {
        let mut value = json!({"name":"read","toolUseId":"a","stop":true});
        value[field] = serde_json::Value::Null;
        let bytes = [
            wire::event_frame("visible"),
            wire::event("toolUseEvent", value),
        ]
        .concat();
        assert!(consume(&mut EventStreamParser::for_api_key(), bytes.clone()).is_err());
        consume(&mut EventStreamParser::default(), bytes).unwrap();
    }
    let mut bytes = event("assistantResponseEvent", br#"{"content":"later"}"#);
    let kind = 12 + 1 + ":message-type".len();
    bytes[kind] = 0; // bool header has no value bytes; keep the rest of the frame structurally valid.
    bytes.drain(kind + 1..kind + 1 + 2 + "event".len());
    let total = bytes.len() as u32;
    let headers = u32::from_be_bytes(bytes[4..8].try_into().unwrap()) - 7;
    bytes[..4].copy_from_slice(&total.to_be_bytes());
    bytes[4..8].copy_from_slice(&headers.to_be_bytes());
    let crc = crc::Crc::<u32>::new(&crc::CRC_32_ISO_HDLC);
    let prelude = crc.checksum(&bytes[..8]);
    bytes[8..12].copy_from_slice(&prelude.to_be_bytes());
    let end = bytes.len() - 4;
    let message = crc.checksum(&bytes[..end]);
    bytes[end..].copy_from_slice(&message.to_be_bytes());
    let bytes = [wire::event_frame("visible"), bytes].concat();
    assert!(consume(&mut EventStreamParser::for_api_key(), bytes.clone()).is_err());
    consume(&mut EventStreamParser::default(), bytes).unwrap();
}

#[test]
fn runtime_integrity_empty_tool_normalization_waits_for_stop_and_preserves_real_arguments() {
    for (parts, expected) in [
        (vec![""], json!({})),
        (vec![" ", "\t", "\u{2003}"], json!({})),
        (vec![" ", "{\"city\":", "\"杭州\"}"], json!({"city":"杭州"})),
        (vec!["{\"value\":\"", " ", "\"}"], json!({"value":" "})),
    ] {
        let mut parser = EventStreamParser::for_api_key();
        let mut output = String::new();
        for (index, input) in parts.iter().enumerate() {
            parser
                .push(Bytes::from(wire::event(
                    "toolUseEvent",
                    json!({"name":"read","toolUseId":"a","input":input,"stop":false}),
                )))
                .unwrap();
            let Some(KiroEvent::ToolUse { input, .. }) = parser.next_event().unwrap() else {
                panic!("tool required")
            };
            output.push_str(&input);
            if parts.iter().all(|s| s.trim().is_empty()) {
                assert!(output.is_empty(), "premature empty JSON at {index}");
            }
            assert!(parser.next_event().unwrap().is_none());
        }
        parser
            .push(Bytes::from(wire::event(
                "toolUseEvent",
                json!({"toolUseId":"a","stop":true}),
            )))
            .unwrap();
        let Some(KiroEvent::ToolUse { input, .. }) = parser.next_event().unwrap() else {
            panic!("stop required")
        };
        output.push_str(&input);
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&output).unwrap(),
            expected
        );
        assert!(parser.next_event().unwrap().is_none());
        parser.finish().unwrap();
    }
}
