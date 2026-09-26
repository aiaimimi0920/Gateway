//! Binary framing and stream lifetime contracts using synthetic upstream SSE.

use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc,
};
use std::task::Poll;

use bytes::Bytes;
use crc::{Crc, CRC_32_ISO_HDLC};
use futures::{stream, StreamExt};
use neuro_gateway::protocol::bedrock_converse::translate_openai_sse_to_bedrock_eventstream;
use serde_json::{json, Value};

fn sse(frames: &[Value]) -> Vec<u8> {
    let mut wire = String::from(": heartbeat\r\n\r\ndata: not-json\r\n\r\n");
    for frame in frames {
        wire.push_str(&format!("data: {frame}\r\n\r\n"));
    }
    wire.push_str("data: [DONE]\r\n\r\n");
    wire.into_bytes()
}

fn decode(frame: &Bytes) -> Value {
    assert!(
        frame.len() >= 16,
        "event-stream frame is shorter than its envelope"
    );
    let u32_at = |offset| u32::from_be_bytes(frame[offset..offset + 4].try_into().unwrap());
    assert_eq!(u32_at(0) as usize, frame.len());
    assert_eq!(u32_at(4), 0, "current bridge emits no event headers");
    let crc = Crc::<u32>::new(&CRC_32_ISO_HDLC);
    assert_eq!(u32_at(8), crc.checksum(&frame[..8]));
    assert_eq!(
        u32_at(frame.len() - 4),
        crc.checksum(&frame[..frame.len() - 4])
    );
    serde_json::from_slice(&frame[12..frame.len() - 4]).unwrap()
}

async fn payloads(wire: &[u8], chunk_size: usize) -> Vec<Value> {
    let chunks: Vec<Result<Bytes, rquest::Error>> = wire
        .chunks(chunk_size)
        .map(|chunk| Ok(Bytes::copy_from_slice(chunk)))
        .collect();
    translate_openai_sse_to_bedrock_eventstream(stream::iter(chunks), "model".into())
        .map(|frame| decode(&frame.unwrap()))
        .collect()
        .await
}

#[tokio::test]
async fn text_usage_and_crc_frames_are_independent_of_utf8_chunk_boundaries() {
    let text = "hello \u{96ea}";
    let wire = sse(&[
        json!({"choices": [{"delta": {"content": text}}]}),
        json!({"choices": [{"delta": {"content": "!"}}]}),
        json!({"choices": [{"delta": {}, "finish_reason": "stop"}],
            "usage": {"prompt_tokens": 2, "completion_tokens": 3, "total_tokens": 5}}),
    ]);
    let expected = vec![
        json!({"messageStart": {"model": "model"}}),
        json!({"contentBlockDelta": {"contentBlockIndex": 0, "delta": {"text": text}}}),
        json!({"contentBlockDelta": {"contentBlockIndex": 0, "delta": {"text": "!"}}}),
        json!({"messageStop": {"stopReason": "end_turn"}}),
        json!({"metadata": {"usage": {"inputTokens": 2, "outputTokens": 3, "totalTokens": 5}}}),
    ];
    for size in [1, 7, wire.len()] {
        assert_eq!(payloads(&wire, size).await, expected, "chunk size {size}");
    }
}

#[tokio::test]
async fn tool_block_starts_once_and_argument_deltas_keep_their_order() {
    let wire = sse(&[
        json!({"choices": [{"delta": {"tool_calls": [{"index": 2, "id": "call-7",
            "function": {"name": "weather", "arguments": "{\"city\":"}}]}}]}),
        json!({"choices": [{"delta": {"tool_calls": [{"index": 2,
            "function": {"name": "weather", "arguments": "\"Paris\"}"}}]}}]}),
        json!({"choices": [{"delta": {}, "finish_reason": "tool_calls"}]}),
    ]);
    let expected = vec![
        json!({"messageStart": {"model": "model"}}),
        json!({"contentBlockStart": {"contentBlockIndex": 2,
            "start": {"toolUse": {"toolUseId": "call-7", "name": "weather"}}}}),
        json!({"contentBlockDelta": {"contentBlockIndex": 2,
            "delta": {"toolUse": {"input": "{\"city\":"}}}}),
        json!({"contentBlockDelta": {"contentBlockIndex": 2,
            "delta": {"toolUse": {"input": "\"Paris\"}"}}}}),
        json!({"messageStop": {"stopReason": "tool_use"}}),
    ];
    for size in [1, 11, wire.len()] {
        assert_eq!(payloads(&wire, size).await, expected, "chunk size {size}");
    }
}

#[tokio::test]
async fn upstream_errors_follow_already_queued_output_without_network_io() {
    let error = rquest::Client::new()
        .get("http://[invalid-host")
        .build()
        .unwrap_err();
    let expected_error = error.to_string();
    let wire = sse(&[json!({"choices": [{"delta": {"content": "before-error"}}]})]);
    let frames = translate_openai_sse_to_bedrock_eventstream(
        stream::iter(vec![Ok(Bytes::from(wire)), Err(error)]),
        "model".into(),
    )
    .collect::<Vec<_>>()
    .await;
    assert_eq!(frames.len(), 3);
    assert_eq!(
        decode(frames[0].as_ref().unwrap()),
        json!({"messageStart": {"model": "model"}})
    );
    assert_eq!(
        decode(frames[1].as_ref().unwrap())["contentBlockDelta"]["delta"]["text"],
        "before-error"
    );
    assert_eq!(frames[2].as_ref().unwrap_err().to_string(), expected_error);
}

struct DropSignal(Arc<AtomicBool>);

impl Drop for DropSignal {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

#[tokio::test]
async fn downstream_drop_releases_upstream_without_eager_polling() {
    let dropped = Arc::new(AtomicBool::new(false));
    let polls = Arc::new(AtomicUsize::new(0));
    let signal = DropSignal(dropped.clone());
    let upstream_polls = polls.clone();
    let wire = Bytes::from(sse(&[
        json!({"choices": [{"delta": {"content": "hello"}}]}),
    ]));
    let upstream = stream::poll_fn(move |_| {
        let _keep_alive = &signal;
        upstream_polls.fetch_add(1, Ordering::SeqCst);
        Poll::Ready(Some(Ok(wire.clone())))
    });
    let mut translated = Box::pin(translate_openai_sse_to_bedrock_eventstream(
        upstream,
        "model".into(),
    ));
    assert_eq!(polls.load(Ordering::SeqCst), 0);
    assert!(translated.next().await.unwrap().is_ok());
    assert_eq!(polls.load(Ordering::SeqCst), 1);
    assert!(!dropped.load(Ordering::SeqCst));
    drop(translated);
    assert!(dropped.load(Ordering::SeqCst));
}

#[tokio::test]
async fn streamed_tool_arguments_preserve_whitespace_and_empty_fragments() {
    let fragments = ["{\"city\":\"", " Paris ", "\"}", ""];
    let frames: Vec<_> = fragments
        .iter()
        .map(|fragment| {
            json!({
                "choices": [{"delta": {"tool_calls": [{"index": 0, "id": "call-7",
                    "function": {"name": "weather", "arguments": fragment}}]}}]
            })
        })
        .collect();
    let wire = sse(&frames);
    for size in [1, wire.len()] {
        let events = payloads(&wire, size).await;
        let inputs: Vec<_> = events
            .iter()
            .filter_map(|event| event["contentBlockDelta"]["delta"]["toolUse"]["input"].as_str())
            .collect();
        assert_eq!(inputs, fragments, "chunk size {size}");
        let input: Value = serde_json::from_str(&inputs.concat()).unwrap();
        assert_eq!(input["city"], " Paris ");
    }
}

async fn expect_terminal_stream_error(wire: Vec<u8>, reason: &str) {
    let mut translated = Box::pin(translate_openai_sse_to_bedrock_eventstream(
        stream::iter([Ok(Bytes::from(wire))]),
        "model".into(),
    ));
    let error = match translated.next().await {
        Some(Err(error)) => error,
        _ => panic!("expected a terminal {reason} error before partial-frame output"),
    };
    assert!(
        error.to_string().contains(reason),
        "unexpected error: {error}"
    );
    assert!(translated.next().await.is_none());
}

#[tokio::test]
async fn unterminated_sse_frame_cannot_exceed_the_64_mib_decoder_budget() {
    expect_terminal_stream_error(vec![b'x'; 64 * 1024 * 1024 + 1], "frame").await;
}

#[tokio::test]
async fn distinct_tool_calls_cannot_exceed_the_4096_entry_budget() {
    let calls: Vec<_> = (0..4097)
        .map(|index| {
            json!({
                "index": index, "id": format!("call-{index}"), "function": {"name": "weather"}
            })
        })
        .collect();
    let wire = sse(&[json!({"choices": [{"delta": {"tool_calls": calls}}]})]);
    expect_terminal_stream_error(wire, "tool call").await;
}

#[tokio::test]
async fn retained_tool_identity_bytes_are_bounded_across_calls() {
    let name = "n".repeat(2 * 1024 * 1024 + 1);
    let wire = sse(&[json!({"choices": [{"delta": {"tool_calls": [
        {"index": 0, "id": "call-0", "function": {"name": name}},
        {"index": 1, "id": "call-1", "function": {"name": name}}
    ]}}]})]);
    expect_terminal_stream_error(wire, "tool state").await;
}

#[tokio::test]
async fn replacing_a_tool_identity_releases_its_retained_byte_budget() {
    let wire = sse(&[
        json!({"choices": [{"delta": {"tool_calls": [{"index": 0, "id": "call-0",
            "function": {"name": "n".repeat(2 * 1024 * 1024)}}]}}]}),
        json!({"choices": [{"delta": {"tool_calls": [{"index": 0,
            "function": {"name": "short"}}]}}]}),
        json!({"choices": [{"delta": {"tool_calls": [{"index": 1, "id": "call-1",
            "function": {"name": "n".repeat(3 * 1024 * 1024)}}]}}]}),
    ]);
    let events = payloads(&wire, wire.len()).await;
    assert_eq!(events.len(), 3);
    assert_eq!(events[2]["contentBlockStart"]["contentBlockIndex"], 1);
}

#[tokio::test]
async fn a_single_frame_cannot_queue_an_unbounded_delta_burst() {
    let call =
        json!({"index": 0, "id": "call-0", "function": {"name": "weather", "arguments": "x"}});
    let wire = sse(&[json!({"choices": [{"delta": {"tool_calls": vec![call; 9000]}}]})]);
    expect_terminal_stream_error(wire, "output event").await;
}

#[tokio::test]
async fn output_envelopes_cannot_exceed_the_64_mib_queue_budget() {
    let empty = json!({"choices": [{"delta": {"content": ""}}]});
    let overhead = format!("data: {empty}\r\n\r\n").len();
    let text = "x".repeat(64 * 1024 * 1024 - overhead);
    let wire = sse(&[json!({"choices": [{"delta": {"content": text}}]})]);
    expect_terminal_stream_error(wire, "output bytes").await;
}

#[tokio::test]
async fn tool_indices_must_fit_the_signed_wire_field() {
    let wire = sse(&[json!({"choices": [{"delta": {"tool_calls": [{
        "index": u64::MAX, "id": "call-0", "function": {"name": "weather"}
    }]}}]})]);
    expect_terminal_stream_error(wire, "tool index").await;
}

#[tokio::test]
async fn transport_error_drops_upstream_without_polling_later_chunks() {
    let dropped = Arc::new(AtomicBool::new(false));
    let polls = Arc::new(AtomicUsize::new(0));
    let signal = DropSignal(dropped.clone());
    let upstream_polls = polls.clone();
    let error = rquest::Client::new()
        .get("http://[invalid-host")
        .build()
        .unwrap_err();
    let wire = sse(&[json!({"choices": [{"delta": {"content": "not-after-error"}}]})]);
    let upstream = stream::iter(vec![Err(error), Ok(Bytes::from(wire))]).map(move |item| {
        let _keep_alive = &signal;
        upstream_polls.fetch_add(1, Ordering::SeqCst);
        item
    });
    let mut translated = Box::pin(translate_openai_sse_to_bedrock_eventstream(
        upstream,
        "model".into(),
    ));
    assert!(translated.next().await.unwrap().is_err());
    assert_eq!(polls.load(Ordering::SeqCst), 1);
    assert!(dropped.load(Ordering::SeqCst));
    assert!(translated.next().await.is_none());
    assert_eq!(polls.load(Ordering::SeqCst), 1);
}
