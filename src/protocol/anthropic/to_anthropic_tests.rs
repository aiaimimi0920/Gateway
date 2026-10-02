use bytes::Bytes;
use futures::StreamExt;
use serde_json::json;

use super::to_anthropic::translate_openai_sse_to_anthropic_with_limit;
use crate::protocol::stream_error_test_support::{
    http_body_error as stream_error, source_messages,
};

fn text_frame(text: &str, line_ending: &str) -> String {
    let payload = json!({
        "choices": [{
            "index": 0,
            "delta": {"content": text},
            "finish_reason": null
        }]
    });
    format!("data: {payload}{line_ending}{line_ending}")
}

fn tool_frame(index: usize, arguments: &str) -> String {
    let payload = json!({
        "choices": [{
            "index": 0,
            "delta": {
                "tool_calls": [{
                    "index": index,
                    "id": format!("call_{index}"),
                    "function": {
                        "name": "lookup",
                        "arguments": arguments
                    }
                }]
            },
            "finish_reason": null
        }]
    });
    format!("data: {payload}\n\n")
}

async fn collect_translation(
    chunks: Vec<Result<Bytes, rquest::Error>>,
    max_frame_bytes: usize,
) -> (Vec<String>, Option<String>) {
    let mut translated = Box::pin(translate_openai_sse_to_anthropic_with_limit(
        futures::stream::iter(chunks),
        "gpt-test".to_string(),
        max_frame_bytes,
    ));
    let mut outputs = Vec::new();
    let mut error = None;
    while let Some(item) = translated.next().await {
        match item {
            Ok(bytes) => outputs.push(String::from_utf8(bytes.to_vec()).expect("UTF-8 event")),
            Err(source) => {
                error = Some(source_messages(&source));
                assert!(
                    translated.next().await.is_none(),
                    "error must terminate stream"
                );
                break;
            }
        }
    }
    (outputs, error)
}

#[tokio::test]
async fn rejects_oversized_unterminated_input_without_terminal_events() {
    let (outputs, error) = collect_translation(vec![Ok(Bytes::from_static(b"123456789"))], 8).await;

    assert!(outputs.is_empty());
    assert!(error
        .expect("oversized frame error")
        .contains("8-byte limit"));
}

#[tokio::test]
async fn yields_completed_frame_before_later_same_chunk_overflow() {
    let frame = text_frame("Hi", "\n");
    let limit = frame.len();
    let input = format!("{frame}{}", "x".repeat(limit + 1));
    let (outputs, error) = collect_translation(vec![Ok(Bytes::from(input))], limit).await;
    let output = outputs.join("");

    assert!(output.contains("event: message_start"));
    assert!(output.contains("\"text\":\"Hi\""));
    assert!(!output.contains("event: message_stop"));
    assert!(error
        .expect("trailing oversized frame error")
        .contains(&format!("{limit}-byte limit")));
}

#[tokio::test]
async fn preserves_chunked_utf8_crlf_and_multiline_data() {
    let frame = concat!(
        "event: update\r\n",
        "data: {\"choices\":[\r\n",
        "data: {\"index\":0,\"delta\":{\"content\":\"你好\"},\"finish_reason\":null}\r\n",
        "data: ]}\r\n\r\n"
    );
    let split = frame.find('你').expect("UTF-8 marker") + 1;
    let bytes = frame.as_bytes();
    let chunks = vec![
        Ok(Bytes::copy_from_slice(&bytes[..split])),
        Ok(Bytes::copy_from_slice(&bytes[split..])),
    ];
    let (outputs, error) = collect_translation(chunks, 4096).await;
    let output = outputs.join("");

    assert!(error.is_none());
    assert!(output.contains("\"text\":\"你好\""));
    assert!(output.contains("event: message_stop"));
}

#[tokio::test]
async fn propagates_upstream_error_without_synthetic_terminal_events() {
    let chunks = vec![
        Ok(Bytes::from(text_frame("Hi", "\n"))),
        Err(stream_error("upstream disconnected").await),
    ];
    let (outputs, error) = collect_translation(chunks, 4096).await;
    let output = outputs.join("");

    assert!(output.contains("\"text\":\"Hi\""));
    assert!(!output.contains("event: message_delta"));
    assert!(!output.contains("event: message_stop"));
    assert!(error
        .expect("upstream error")
        .contains("upstream disconnected"));
}

#[tokio::test]
async fn eof_flushes_parsed_fields_then_emits_terminal_sequence_once() {
    let payload = json!({
        "choices": [{
            "index": 0,
            "delta": {"content": "Hi"},
            "finish_reason": null
        }]
    });
    let input = format!("data: {payload}\nunfinished");
    let (outputs, error) = collect_translation(vec![Ok(Bytes::from(input))], 4096).await;
    let event_names: Vec<&str> = outputs
        .iter()
        .filter_map(|event| event.lines().next()?.strip_prefix("event: "))
        .collect();

    assert!(error.is_none());
    assert_eq!(
        event_names,
        [
            "message_start",
            "content_block_start",
            "content_block_delta",
            "content_block_stop",
            "message_delta",
            "message_stop"
        ]
    );
}

#[tokio::test]
async fn reopens_a_reused_tool_index_after_an_intervening_text_block() {
    let input = format!(
        "{}{}{}",
        tool_frame(0, "{\"a\":"),
        text_frame("between", "\n"),
        tool_frame(0, "1}")
    );
    let (outputs, error) = collect_translation(vec![Ok(Bytes::from(input))], 4096).await;
    let output = outputs.join("");

    assert!(error.is_none());
    assert_eq!(output.matches("event: content_block_start").count(), 3);
    assert_eq!(output.matches("\"type\":\"tool_use\"").count(), 2);
    assert_eq!(output.matches("\"partial_json\"").count(), 2);
}

#[tokio::test]
async fn rejects_unbounded_tool_block_growth_without_synthetic_completion() {
    let input = (0..=4096)
        .map(|index| tool_frame(index, "{}"))
        .collect::<String>();
    let (outputs, error) = collect_translation(vec![Ok(Bytes::from(input))], 4096).await;
    let output = outputs.join("");

    assert!(error
        .expect("tool block limit error")
        .contains("4096-content-block limit"));
    assert!(!output.contains("event: message_stop"));
}
