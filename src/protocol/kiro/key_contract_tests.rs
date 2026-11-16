//! Key admission is strict without changing the legacy OAuth compatibility contract.
use super::{event_stream::EventStreamParser, event_stream_tests::event_frame};
use crate::{error::GatewayError, protocol::canonical::CanonicalRelayRequest};
use bytes::Bytes;
use serde_json::{json, Value};

fn consume(parser: &mut EventStreamParser, frames: Vec<Vec<u8>>) -> Result<(), GatewayError> {
    for frame in frames {
        parser.push(Bytes::from(frame))?;
        while parser.next_event()?.is_some() {}
    }
    parser.finish()
}
fn tool(value: Value) -> Vec<u8> {
    event_frame("toolUseEvent", value)
}

#[test]
fn key_response_accepts_compact_continuations_and_stop_only_frame() {
    let frames = vec![
        tool(json!({"name":"read","toolUseId":"a"})),
        tool(json!({"toolUseId":"a","input":"{\"city\":\"杭州\"}"})),
        tool(json!({"toolUseId":"a","stop":true})),
    ];
    consume(&mut EventStreamParser::for_api_key(), frames).unwrap();
}

#[test]
fn key_response_rejects_bad_fields_identity_changes_reuse_and_incomplete_tools_only() {
    for frames in [
        vec![event_frame("assistantResponseEvent", json!({"content":1}))],
        vec![event_frame(
            "contextUsageEvent",
            json!({"contextUsagePercentage":"100"}),
        )],
        vec![tool(json!({}))],
        vec![tool(json!({"name":"read","toolUseId":"a","input":"{}"}))],
        vec![
            tool(json!({"name":"read","toolUseId":"a"})),
            tool(json!({"name":"changed","toolUseId":"a","input":"{}","stop":true})),
        ],
        vec![
            tool(json!({"name":"read","toolUseId":"a","input":"{}","stop":true})),
            tool(json!({"toolUseId":"a","input":"{}"})),
        ],
        vec![tool(
            json!({"name":"read","toolUseId":"a","input":"[]","stop":true}),
        )],
    ] {
        assert!(consume(&mut EventStreamParser::for_api_key(), frames.clone()).is_err());
        // These exact old permissive results are intentional compatibility, not Key success.
        consume(&mut EventStreamParser::default(), frames).unwrap();
    }
}

fn request(messages: Value) -> CanonicalRelayRequest {
    crate::protocol::openai::normalize_chat_completions(json!({"model":"exact","messages":messages,
        "tools":[{"type":"function","function":{"name":"read","parameters":{"type":"object","properties":{}}}}]})).unwrap()
}

#[test]
fn key_request_refuses_dropped_image_schemes_and_incomplete_or_duplicate_history() {
    for messages in [
        json!([{"role":"user","content":"before"},{"role":"assistant","content":"prefix"}]),
        json!([{"role":"user","content":[{"type":"image_url","image_url":{"url":"DATA:image/png;base64,aGk="}}]}]),
        json!([{"role":"tool","content":"orphan","tool_call_id":"missing"}]),
        json!([{"role":"assistant","tool_calls":[{"type":"function","function":{"name":"read","arguments":"{}"}}]},{"role":"user","content":"now"}]),
        json!([{"role":"assistant","tool_calls":[{"id":"a","type":"function","function":{"name":"read","arguments":"{bad"}}]},{"role":"tool","tool_call_id":"a","content":"result"}]),
        json!([{"role":"assistant","tool_calls":[{"id":"a","type":"function","function":{"name":"read","arguments":"{}"}},{"id":"a","type":"function","function":{"name":"read","arguments":"{}"}}]},{"role":"tool","tool_call_id":"a","content":"result"}]),
    ] {
        let request = request(messages);
        let error = super::pack_kiro_api_key(&request, "exact").unwrap_err();
        assert_eq!(error.http_status, Some(400));
        assert_eq!(
            error.code.as_deref(),
            Some("kiro_api_key_unsupported_input")
        );
    }
}

#[test]
fn key_messages_base64_image_is_preserved_and_malformed_sources_fail() {
    let body = |source: Value| {
        json!({"model":"exact","max_tokens":32,
        "messages":[{"role":"user","content":[{"type":"image","source":source}]}]})
    };
    let request = crate::protocol::anthropic::normalize_messages(body(
        json!({"type":"base64","media_type":"image/png","data":"aGk="}),
    ))
    .unwrap();
    let packed = super::pack_kiro_api_key(&request, "exact").unwrap();
    assert_eq!(
        packed["conversationState"]["currentMessage"]["userInputMessage"]["images"][0]["source"]
            ["bytes"],
        "aGk="
    );
    for source in [
        json!({"type":"base64","data":"aGk="}),
        json!({"type":"base64","media_type":"image/png"}),
    ] {
        assert!(crate::protocol::anthropic::normalize_messages(body(source)).is_err());
    }
}

#[test]
fn key_responses_data_image_is_typed_but_rich_or_file_images_remain_explicitly_unsupported() {
    let request = |part: Value| {
        crate::protocol::responses::normalize_responses(json!({"model":"exact",
        "input":[{"role":"user","content":[part]}]}))
        .unwrap()
    };
    let part =
        json!({"type":"input_image","image_url":"data:image/png;base64,aGk=","detail":"auto"});
    let req = request(part.clone());
    let packed = super::pack_kiro_api_key(&req, "exact").unwrap();
    assert_eq!(
        packed["conversationState"]["currentMessage"]["userInputMessage"]["images"][0]["source"]
            ["bytes"],
        "aGk="
    );
    for rich in [
        json!({"type":"input_image","file_id":"file-unsupported"}),
        json!({"type":"input_image","image_url":"data:image/png;base64,aGk=","unknown":"must-not-be-dropped"}),
    ] {
        let req = request(rich.clone());
        assert!(super::pack_kiro_api_key(&req, "exact").is_err());
        assert!(
            matches!(&req.messages[0].content[0], crate::protocol::canonical::ContentPart::Raw {value} if value == &rich)
        );
    }
    for detail in [json!(123), Value::Null] {
        let rich =
            json!({"type":"input_image","image_url":"data:image/png;base64,aGk=","detail":detail});
        let req = request(rich.clone());
        assert!(super::pack_kiro_api_key(&req, "exact").is_err());
        assert!(
            matches!(&req.messages[0].content[0], crate::protocol::canonical::ContentPart::Raw {value} if value == &rich)
        );
    }
    let explicit = request(
        json!({"type":"input_image","image_url":"data:image/png;base64,aGk=","detail":"high"}),
    );
    assert!(super::pack_kiro_api_key(&explicit, "exact").is_err());
}
