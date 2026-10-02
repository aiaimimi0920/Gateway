use bytes::Bytes;
use futures::StreamExt;
use serde_json::{json, Value};

use super::stream_translate::translate_openai_chat_sse_to_legacy_completions_with_limit;
use super::translate_openai_chat_sse_to_legacy_completions;
use crate::protocol::stream_error::StreamError;
use crate::protocol::stream_error_test_support::{
    http_body_error as synthetic_stream_error, source_messages,
};

async fn translate(
    chunks: Vec<Result<Bytes, rquest::Error>>,
) -> Vec<Result<Bytes, StreamError<rquest::Error>>> {
    translate_openai_chat_sse_to_legacy_completions(
        futures::stream::iter(chunks),
        "requested-model".to_string(),
    )
    .collect()
    .await
}

async fn translate_with_limit(
    chunks: Vec<Result<Bytes, rquest::Error>>,
    max_frame_bytes: usize,
) -> Vec<Result<Bytes, StreamError<rquest::Error>>> {
    translate_openai_chat_sse_to_legacy_completions_with_limit(
        futures::stream::iter(chunks),
        "requested-model".to_string(),
        max_frame_bytes,
    )
    .collect()
    .await
}

fn chat_text_frame(text: &str, line_ending: &str) -> String {
    let payload = json!({
        "choices": [{
            "delta": {"content": text},
            "finish_reason": null
        }]
    });
    format!("data: {payload}{line_ending}{line_ending}")
}

fn output_data(output: &Bytes) -> &str {
    std::str::from_utf8(output)
        .expect("translated SSE must be UTF-8")
        .lines()
        .find_map(|line| line.strip_prefix("data: "))
        .expect("translated SSE must contain data")
}

fn output_json(output: &Bytes) -> Value {
    serde_json::from_str(output_data(output)).expect("translated data must be JSON")
}

#[tokio::test]
async fn native_legacy_frames_are_passed_through_in_order() {
    let native = concat!(
        "event: completion\n",
        "data: {\"object\":\"text_completion.chunk\",\"choices\":[{\"text\":\"hi\"}]}\n\n",
        "data: [DONE]\n\n"
    );

    let output = translate(vec![Ok(Bytes::from_static(native.as_bytes()))]).await;

    assert_eq!(output.len(), 2);
    let joined = output
        .into_iter()
        .map(Result::unwrap)
        .fold(Vec::new(), |mut joined, chunk| {
            joined.extend_from_slice(&chunk);
            joined
        });
    assert_eq!(joined, native.as_bytes());
}

#[tokio::test]
async fn split_utf8_chat_frames_preserve_text_model_usage_and_finish_reason() {
    let input = concat!(
        "data: {\"model\":\"upstream-model\",\"choices\":[{\"delta\":{\"content\":\"你\"},\"finish_reason\":null}]}\r\n\r\n",
        "data: {\"choices\":[{\"delta\":{\"content\":\"好\"},\"finish_reason\":\"length\"}],\"usage\":{\"prompt_tokens\":4,\"completion_tokens\":2,\"total_tokens\":6,\"input_tokens_details\":{\"cached_tokens\":3}}}\n\n"
    );
    let bytes = input.as_bytes();
    let split = bytes
        .windows("你".len())
        .position(|window| window == "你".as_bytes())
        .expect("fixture must contain multibyte text")
        + 1;

    let output = translate(vec![
        Ok(Bytes::copy_from_slice(&bytes[..split])),
        Ok(Bytes::copy_from_slice(&bytes[split..])),
    ])
    .await;

    assert_eq!(output.len(), 4);
    let first = output_json(output[0].as_ref().unwrap());
    let second = output_json(output[1].as_ref().unwrap());
    let final_chunk = output_json(output[2].as_ref().unwrap());
    assert_eq!(first["choices"][0]["text"], "你");
    assert_eq!(second["choices"][0]["text"], "好");
    assert_eq!(first["model"], "upstream-model");
    assert_eq!(final_chunk["model"], "upstream-model");
    assert_eq!(final_chunk["choices"][0]["finish_reason"], "length");
    assert_eq!(final_chunk["usage"]["total_tokens"], 6);
    assert_eq!(final_chunk["usage"]["input_tokens_details"], Value::Null);
    assert_eq!(output_data(output[3].as_ref().unwrap()), "[DONE]");
}

#[tokio::test]
async fn usage_after_finish_is_retained_until_done() {
    let finish = json!({
        "choices": [{"delta": {}, "finish_reason": "stop"}]
    });
    let usage = json!({
        "choices": [],
        "usage": {"input_tokens": 7, "output_tokens": 5}
    });
    let input = format!("data: {finish}\n\ndata: {usage}\n\ndata: [DONE]\n\n");

    let output = translate(vec![Ok(Bytes::from(input))]).await;

    assert_eq!(output.len(), 2);
    let final_chunk = output_json(output[0].as_ref().unwrap());
    assert_eq!(final_chunk["choices"][0]["finish_reason"], "stop");
    assert_eq!(final_chunk["usage"]["prompt_tokens"], 7);
    assert_eq!(final_chunk["usage"]["completion_tokens"], 5);
    assert_eq!(final_chunk["usage"]["total_tokens"], 12);
    assert_eq!(output_data(output[1].as_ref().unwrap()), "[DONE]");
}

#[tokio::test]
async fn eof_without_done_emits_one_final_chunk_and_done() {
    let input = concat!(
        "data: {\"choices\":[{\"delta\":{\"content\":\"partial\"},\"finish_reason\":null}]}\n\n"
    );

    let output = translate(vec![Ok(Bytes::from_static(input.as_bytes()))]).await;

    assert_eq!(output.len(), 3);
    assert_eq!(
        output_json(output[0].as_ref().unwrap())["choices"][0]["text"],
        "partial"
    );
    assert_eq!(
        output_json(output[1].as_ref().unwrap())["choices"][0]["finish_reason"],
        "stop"
    );
    assert_eq!(output_data(output[2].as_ref().unwrap()), "[DONE]");
}

#[tokio::test]
async fn malformed_frame_is_ignored_without_reordering_later_content() {
    let input = concat!(
        "data: {not-json}\n\n",
        "data: {\"choices\":[{\"delta\":{\"content\":\"valid\"},\"finish_reason\":null}]}\n\n",
        "data: [DONE]\n\n"
    );

    let output = translate(vec![Ok(Bytes::from_static(input.as_bytes()))]).await;

    assert_eq!(output.len(), 3);
    assert_eq!(
        output_json(output[0].as_ref().unwrap())["choices"][0]["text"],
        "valid"
    );
    assert_eq!(output_data(output[2].as_ref().unwrap()), "[DONE]");
}

#[tokio::test]
async fn upstream_error_is_forwarded_without_synthetic_terminal_event() {
    let output = translate(vec![
        Ok(Bytes::from(chat_text_frame("before-error", "\n"))),
        Err(synthetic_stream_error("upstream failed").await),
    ])
    .await;

    assert_eq!(output.len(), 2);
    assert_eq!(
        output_json(output[0].as_ref().unwrap())["choices"][0]["text"],
        "before-error"
    );
    let error = output[1].as_ref().expect_err("error must be forwarded");
    assert!(matches!(error, StreamError::Transport(_)));
    assert!(source_messages(error).contains("upstream failed"));
}

#[tokio::test]
async fn rejects_oversized_unterminated_input_without_terminal_events() {
    let output = translate_with_limit(vec![Ok(Bytes::from_static(b"123456789"))], 8).await;

    assert_eq!(output.len(), 1);
    let error = output[0]
        .as_ref()
        .expect_err("oversized frame must fail translation");
    assert!(error.to_string().contains("8-byte limit"));
}

#[tokio::test]
async fn yields_completed_frame_before_later_same_chunk_overflow() {
    let frame = chat_text_frame("before-overflow", "\n");
    let limit = frame.len();
    let input = format!("{frame}{}", "x".repeat(limit + 1));

    let output = translate_with_limit(vec![Ok(Bytes::from(input))], limit).await;

    assert_eq!(output.len(), 2);
    assert_eq!(
        output_json(output[0].as_ref().unwrap())["choices"][0]["text"],
        "before-overflow"
    );
    let error = output[1]
        .as_ref()
        .expect_err("trailing oversized frame must fail translation");
    assert!(error.to_string().contains(&format!("{limit}-byte limit")));
}

#[tokio::test]
async fn bounded_decoder_preserves_split_utf8_crlf_and_multiline_data() {
    let frame = concat!(
        "event: update\r\n",
        "data: {\"choices\":[\r\n",
        "data: {\"delta\":{\"content\":\"你好\"},\"finish_reason\":null}\r\n",
        "data: ]}\r\n\r\n"
    );
    let split = frame.find('你').expect("fixture must contain UTF-8 text") + 1;
    let bytes = frame.as_bytes();

    let output = translate_with_limit(
        vec![
            Ok(Bytes::copy_from_slice(&bytes[..split])),
            Ok(Bytes::copy_from_slice(&bytes[split..])),
        ],
        4096,
    )
    .await;

    assert_eq!(output.len(), 3);
    assert_eq!(
        output_json(output[0].as_ref().unwrap())["choices"][0]["text"],
        "你好"
    );
    assert_eq!(output_data(output[2].as_ref().unwrap()), "[DONE]");
}

#[tokio::test]
async fn done_stops_before_trailing_frames_from_the_same_chunk() {
    let input = format!(
        "{}data: [DONE]\n\n{}",
        chat_text_frame("before-done", "\n"),
        chat_text_frame("after-done", "\n")
    );

    let output = translate(vec![Ok(Bytes::from(input))]).await;

    assert_eq!(output.len(), 3);
    assert_eq!(
        output_json(output[0].as_ref().unwrap())["choices"][0]["text"],
        "before-done"
    );
    assert_eq!(output_data(output[2].as_ref().unwrap()), "[DONE]");
}
