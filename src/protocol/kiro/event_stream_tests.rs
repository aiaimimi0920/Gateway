use std::collections::HashMap;

use bytes::Bytes;
use crc::{Crc, CRC_32_ISO_HDLC};
use futures::StreamExt;
use serde_json::json;

use super::event_stream::{parse_event_stream_frame, parse_headers, EventStreamParser, KiroEvent};
use super::{
    translate_kiro_event_stream_to_anthropic_sse, translate_kiro_event_stream_to_openai_sse,
};
use crate::protocol::canonical::{
    CanonicalMessage, CanonicalRelayRequest, ContentPart, EndpointKind, MessageRole, ProtocolFamily,
};

const CRC: Crc<u32> = Crc::<u32>::new(&CRC_32_ISO_HDLC);

fn string_header(name: &str, value: &str) -> Vec<u8> {
    let mut header = Vec::new();
    header.push(u8::try_from(name.len()).expect("test header name must fit"));
    header.extend_from_slice(name.as_bytes());
    header.push(7);
    header.extend_from_slice(
        &u16::try_from(value.len())
            .expect("test header value must fit")
            .to_be_bytes(),
    );
    header.extend_from_slice(value.as_bytes());
    header
}

fn typed_header(name: &str, value_type: u8, value: &[u8]) -> Vec<u8> {
    let mut header = Vec::new();
    header.push(u8::try_from(name.len()).expect("test header name must fit"));
    header.extend_from_slice(name.as_bytes());
    header.push(value_type);
    header.extend_from_slice(value);
    header
}

fn frame(headers: &[u8], payload: &[u8]) -> Vec<u8> {
    let total_len = 16usize
        .checked_add(headers.len())
        .and_then(|len| len.checked_add(payload.len()))
        .expect("test frame length must fit");
    let mut frame = Vec::with_capacity(total_len);
    frame.extend_from_slice(
        &u32::try_from(total_len)
            .expect("test frame total must fit")
            .to_be_bytes(),
    );
    frame.extend_from_slice(
        &u32::try_from(headers.len())
            .expect("test header total must fit")
            .to_be_bytes(),
    );
    frame.extend_from_slice(&CRC.checksum(&frame).to_be_bytes());
    frame.extend_from_slice(headers);
    frame.extend_from_slice(payload);
    frame.extend_from_slice(&CRC.checksum(&frame).to_be_bytes());
    frame
}

pub(super) fn event_frame(event_type: &str, payload: serde_json::Value) -> Vec<u8> {
    let mut headers = string_header(":message-type", "event");
    headers.extend_from_slice(&string_header(":event-type", event_type));
    frame(&headers, payload.to_string().as_bytes())
}

fn corrupted_prelude_frame() -> Vec<u8> {
    let mut invalid = event_frame("assistantResponseEvent", json!({"content":"bad"}));
    invalid[8] ^= 0x01;
    invalid
}

pub(super) fn provider_error_frame() -> Vec<u8> {
    let mut headers = string_header(":message-type", "error");
    headers.extend_from_slice(&string_header(":error-code", "ProviderFailure"));
    frame(&headers, b"provider failed")
}

fn content_length_exception_frame() -> Vec<u8> {
    let mut headers = string_header(":message-type", "exception");
    headers.extend_from_slice(&string_header(
        ":exception-type",
        "ContentLengthExceededException",
    ));
    frame(&headers, b"context full")
}

pub(super) fn request() -> CanonicalRelayRequest {
    CanonicalRelayRequest {
        protocol_family: ProtocolFamily::OpenAi,
        endpoint_kind: EndpointKind::ChatCompletions,
        requested_model: Some("claude-sonnet-4.6".to_string()),
        stream: true,
        messages: vec![CanonicalMessage {
            role: MessageRole::User,
            content: vec![ContentPart::Text {
                text: "hello".to_string(),
            }],
            name: None,
            tool_call_id: None,
            tool_calls: Vec::new(),
        }],
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

async fn openai_output(chunks: Vec<Bytes>) -> String {
    let output = translate_kiro_event_stream_to_openai_sse(
        futures::stream::iter(chunks.into_iter().map(Ok)),
        "claude-sonnet-4.6".to_string(),
        request(),
    )
    .collect::<Vec<_>>()
    .await
    .into_iter()
    .map(Result::unwrap)
    .fold(Vec::new(), |mut output, chunk| {
        output.extend_from_slice(&chunk);
        output
    });
    String::from_utf8(output).expect("OpenAI SSE must be UTF-8")
}

async fn anthropic_output(chunks: Vec<Bytes>) -> String {
    let output = translate_kiro_event_stream_to_anthropic_sse(
        futures::stream::iter(chunks.into_iter().map(Ok)),
        "claude-sonnet-4.6".to_string(),
        request(),
    )
    .collect::<Vec<_>>()
    .await
    .into_iter()
    .map(Result::unwrap)
    .fold(Vec::new(), |mut output, chunk| {
        output.extend_from_slice(&chunk);
        output
    });
    String::from_utf8(output).expect("Anthropic SSE must be UTF-8")
}

#[test]
fn event_stream_parser_preserves_split_and_multi_frame_order() {
    let first = event_frame("assistantResponseEvent", json!({"content":"one"}));
    let second = event_frame("assistantResponseEvent", json!({"content":"two"}));
    let split = first.len() / 2;
    let mut parser = EventStreamParser::default();

    parser
        .push(Bytes::copy_from_slice(&first[..split]))
        .unwrap();
    assert!(parser.next_event().unwrap().is_none());
    let mut tail = first[split..].to_vec();
    tail.extend_from_slice(&second);
    parser.push(Bytes::from(tail)).unwrap();
    let events = [
        parser.next_event().unwrap().unwrap(),
        parser.next_event().unwrap().unwrap(),
    ];

    assert_eq!(events.len(), 2);
    assert!(matches!(
        &events[0],
        KiroEvent::AssistantResponse { content } if content == "one"
    ));
    assert!(matches!(
        &events[1],
        KiroEvent::AssistantResponse { content } if content == "two"
    ));
    assert!(parser.next_event().unwrap().is_none());
    parser.finish().unwrap();
}

#[test]
fn event_stream_parser_preserves_crc_before_header_length_errors() {
    let mut invalid = frame(&[], b"{}");
    invalid[4..8].copy_from_slice(&u32::MAX.to_be_bytes());

    let error = parse_event_stream_frame(&invalid).unwrap_err();
    assert_eq!(error.code.as_deref(), Some("kiro_invalid_prelude_crc"));

    let prelude_crc = CRC.checksum(&invalid[..8]);
    invalid[8..12].copy_from_slice(&prelude_crc.to_be_bytes());
    let error = parse_event_stream_frame(&invalid).unwrap_err();
    assert_eq!(error.code.as_deref(), Some("kiro_invalid_message_crc"));

    let message_crc = CRC.checksum(&invalid[..invalid.len() - 4]);
    let trailer = invalid.len() - 4;
    invalid[trailer..].copy_from_slice(&message_crc.to_be_bytes());
    let error = parse_event_stream_frame(&invalid).unwrap_err();
    assert_eq!(error.code.as_deref(), Some("kiro_invalid_header_length"));
}

#[test]
fn event_stream_parser_rejects_declared_frame_over_limit() {
    let mut prelude = Vec::from(u32::MAX.to_be_bytes());
    prelude.extend_from_slice(&0u32.to_be_bytes());
    prelude.extend_from_slice(&CRC.checksum(&prelude).to_be_bytes());

    let mut parser = EventStreamParser::default();
    parser.push(Bytes::from(prelude)).unwrap();
    let error = parser
        .next_event()
        .expect_err("oversized declared frame must fail immediately");
    assert_eq!(
        error.code.as_deref(),
        Some("kiro_event_stream_frame_too_large")
    );
}

#[test]
fn event_stream_parser_bounds_buffer_and_clears_all_failure_state() {
    let mut parser = EventStreamParser::test_with_max_buffer_bytes(8);
    parser.push(Bytes::from_static(b"123456789")).unwrap();
    let error = parser
        .next_event()
        .expect_err("oversized pending buffer must fail");
    assert_eq!(
        error.code.as_deref(),
        Some("kiro_event_stream_buffer_too_large")
    );
    assert_eq!(parser.test_buffered_len(), 0);

    parser.push(Bytes::from_static(b"1234")).unwrap();
    let error = parser.finish().expect_err("partial frame must fail at EOF");
    assert_eq!(error.code.as_deref(), Some("kiro_truncated_event_stream"));
    assert_eq!(parser.test_buffered_len(), 0);
}

#[test]
fn event_stream_parser_recovers_after_crc_error() {
    let mut parser = EventStreamParser::default();
    parser.push(Bytes::from(corrupted_prelude_frame())).unwrap();
    let error = parser.next_event().expect_err("CRC mismatch must fail");
    assert_eq!(error.code.as_deref(), Some("kiro_invalid_prelude_crc"));
    assert_eq!(parser.test_buffered_len(), 0);

    parser
        .push(Bytes::from(event_frame(
            "assistantResponseEvent",
            json!({"content":"recovered"}),
        )))
        .unwrap();
    assert!(matches!(
        parser.next_event().unwrap(),
        Some(KiroEvent::AssistantResponse { content }) if content == "recovered"
    ));
}

#[test]
fn event_stream_parser_rejects_truncated_header_values_without_panicking() {
    for value_type in [2u8, 3, 4, 5, 6, 7, 8, 9] {
        let headers = [1, b'x', value_type];
        let error = parse_headers(&headers).expect_err("truncated header value must fail");
        assert_eq!(
            error.code.as_deref(),
            Some("kiro_invalid_header_overflow"),
            "header type {value_type}"
        );
    }
}

#[test]
fn event_stream_parser_preserves_all_header_types_and_boundary_errors() {
    let mut headers = Vec::new();
    headers.extend_from_slice(&typed_header("bool0", 0, &[]));
    headers.extend_from_slice(&typed_header("bool1", 1, &[]));
    headers.extend_from_slice(&typed_header("byte", 2, &[1]));
    headers.extend_from_slice(&typed_header("short", 3, &[0; 2]));
    headers.extend_from_slice(&typed_header("int", 4, &[0; 4]));
    headers.extend_from_slice(&typed_header("long", 5, &[0; 8]));
    headers.extend_from_slice(&typed_header("bytes", 6, &[0, 2, 1, 2]));
    headers.extend_from_slice(&typed_header("string", 7, &[0, 2, b'o', b'k']));
    headers.extend_from_slice(&typed_header("time", 8, &[0; 8]));
    headers.extend_from_slice(&typed_header("uuid", 9, &[0; 16]));
    assert_eq!(parse_headers(&headers).unwrap().len(), 10);

    let bytes_error = parse_headers(&typed_header("bytes", 6, &[0, 2, 1])).unwrap_err();
    assert_eq!(
        bytes_error.code.as_deref(),
        Some("kiro_invalid_header_overflow")
    );
    let string_error = parse_headers(&typed_header("string", 7, &[0, 2, b'x'])).unwrap_err();
    assert_eq!(
        string_error.code.as_deref(),
        Some("kiro_invalid_header_string_bounds")
    );
    assert_eq!(
        parse_headers(&[4, b'x']).unwrap_err().code.as_deref(),
        Some("kiro_invalid_header_bounds")
    );
    assert_eq!(
        parse_headers(&[1, b'x']).unwrap_err().code.as_deref(),
        Some("kiro_invalid_header_type")
    );
    assert_eq!(
        parse_headers(&[1, b'x', 10]).unwrap_err().code.as_deref(),
        Some("kiro_unsupported_header_type")
    );
}

#[test]
fn event_stream_parser_accepts_a_frame_at_the_exact_limit() {
    let input = event_frame("assistantResponseEvent", json!({"content":"exact"}));
    let mut parser = EventStreamParser::test_with_max_buffer_bytes(input.len());

    parser.push(Bytes::from(input)).unwrap();
    assert!(matches!(
        parser.next_event().unwrap(),
        Some(KiroEvent::AssistantResponse { content }) if content == "exact"
    ));
    parser.finish().unwrap();
}

#[test]
fn event_stream_parser_delivers_valid_frame_before_same_chunk_overflow() {
    let first = event_frame("assistantResponseEvent", json!({"content":"first"}));
    let mut input = first.clone();
    input.extend_from_slice(&u32::MAX.to_be_bytes());
    input.extend_from_slice(&[0; 8]);
    let mut parser = EventStreamParser::test_with_max_buffer_bytes(first.len());
    parser.push(Bytes::from(input)).unwrap();
    assert!(matches!(parser.next_event().unwrap(),
        Some(KiroEvent::AssistantResponse { content }) if content == "first"));
    assert_eq!(
        parser.next_event().unwrap_err().code.as_deref(),
        Some("kiro_event_stream_frame_too_large")
    );
    assert_eq!(parser.test_buffered_len(), 0);
}

#[test]
fn event_stream_parser_limit_is_per_frame_not_transport_chunk() {
    let first = event_frame("assistantResponseEvent", json!({"content":"first"}));
    let input = first.repeat(3);
    let mut parser = EventStreamParser::test_with_max_buffer_bytes(first.len());
    parser.push(Bytes::from(input)).unwrap();
    for _ in 0..3 {
        assert!(matches!(parser.next_event().unwrap(),
            Some(KiroEvent::AssistantResponse { content }) if content == "first"));
    }
    assert!(parser.next_event().unwrap().is_none());
    parser.finish().unwrap();
}

#[tokio::test]
async fn openai_yields_valid_event_before_same_chunk_error_without_success_terminal() {
    let mut input = event_frame("assistantResponseEvent", json!({"content":"before-error"}));
    input.extend_from_slice(&corrupted_prelude_frame());

    let output = openai_output(vec![Bytes::from(input)]).await;

    assert!(output.contains("before-error"));
    assert!(output.contains("Invalid Kiro event-stream prelude CRC"));
    assert!(!output.contains("[DONE]"));
    assert!(!output.contains("\"finish_reason\":\"stop\""));
}

#[tokio::test]
async fn openai_translation_continues_across_chunks_before_eof_finish() {
    let output = openai_output(vec![
        Bytes::from(event_frame(
            "assistantResponseEvent",
            json!({"content":"first-chunk"}),
        )),
        Bytes::from(event_frame(
            "assistantResponseEvent",
            json!({"content":"second-chunk"}),
        )),
    ])
    .await;

    let first = output
        .find("first-chunk")
        .expect("first event must be output");
    let second = output
        .find("second-chunk")
        .expect("second event must be output");
    let done = output.find("[DONE]").expect("clean EOF must finish");
    assert!(first < second && second < done);
    assert_eq!(output.matches("[DONE]").count(), 1);
}

#[tokio::test]
async fn anthropic_translation_continues_across_chunks_before_eof_finish() {
    let output = anthropic_output(vec![
        Bytes::from(event_frame(
            "assistantResponseEvent",
            json!({"content":"first-chunk"}),
        )),
        Bytes::from(event_frame(
            "assistantResponseEvent",
            json!({"content":"second-chunk"}),
        )),
    ])
    .await;

    let first = output
        .find("first-chunk")
        .expect("first event must be output");
    let second = output
        .find("second-chunk")
        .expect("second event must be output");
    let stop = output.find("message_stop").expect("clean EOF must finish");
    assert!(first < second && second < stop);
    assert_eq!(output.matches("event: message_stop").count(), 1);
}

#[tokio::test]
async fn openai_rejects_truncated_eof_without_success_terminal() {
    let complete = event_frame("assistantResponseEvent", json!({"content":"truncated"}));
    let output = openai_output(vec![Bytes::copy_from_slice(&complete[..12])]).await;

    assert!(output.contains("Truncated Kiro event-stream frame"));
    assert!(!output.contains("[DONE]"));
    assert!(!output.contains("\"finish_reason\":\"stop\""));
}

#[tokio::test]
async fn anthropic_parser_error_does_not_emit_message_stop() {
    let output = anthropic_output(vec![Bytes::from(corrupted_prelude_frame())]).await;

    assert!(output.contains("Invalid Kiro event-stream prelude CRC"));
    assert!(!output.contains("message_stop"));
    assert!(!output.contains("\"stop_reason\""));
}

#[tokio::test]
async fn openai_provider_error_event_does_not_emit_success_terminal() {
    let output = openai_output(vec![Bytes::from(provider_error_frame())]).await;

    assert!(output.contains("provider failed"));
    assert!(output.contains("ProviderFailure"));
    assert!(!output.contains("[DONE]"));
    assert!(!output.contains("\"finish_reason\":\"stop\""));
}

#[tokio::test]
async fn anthropic_provider_error_event_does_not_emit_message_stop() {
    let output = anthropic_output(vec![Bytes::from(provider_error_frame())]).await;

    assert!(output.contains("ProviderFailure: provider failed"));
    assert!(!output.contains("message_stop"));
    assert!(!output.contains("event: message_delta"));
}

#[tokio::test]
async fn content_length_exception_retains_length_terminal_semantics() {
    let openai = openai_output(vec![Bytes::from(content_length_exception_frame())]).await;
    assert!(openai.contains("\"finish_reason\":\"length\""));
    assert!(openai.contains("[DONE]"));

    let anthropic = anthropic_output(vec![Bytes::from(content_length_exception_frame())]).await;
    assert!(anthropic.contains("\"stop_reason\":\"max_tokens\""));
    assert!(anthropic.contains("event: message_stop"));
}
