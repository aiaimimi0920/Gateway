use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use bytes::Bytes;
use futures::Stream;
use neuro_gateway::protocol::canonical::TokenUsage;
use neuro_gateway::upstream::stream::{
    snapshot_tapped_completion_semantics, snapshot_tapped_usage, tap_sse_completion_semantics,
    tap_sse_usage,
};

#[path = "stream_observation_limits/allocation.rs"]
mod allocation;

const FRAME_BYTES: usize = 64 * 1024 * 1024;
const FRAME_LINES: usize = 65_536;

struct Observation {
    stream: Pin<Box<dyn Stream<Item = Result<Bytes, rquest::Error>> + Send>>,
    usage: Arc<Mutex<Option<TokenUsage>>>,
    completion: Arc<Mutex<Option<String>>>,
}

impl Observation {
    fn new(chunks: Vec<Bytes>) -> Self {
        let (stream, usage) = tap_sse_usage(futures::stream::iter(chunks.into_iter().map(Ok)));
        let (stream, completion) = tap_sse_completion_semantics(stream);
        Self {
            stream: Box::pin(stream),
            usage,
            completion,
        }
    }

    fn next(&mut self) -> Option<Bytes> {
        let mut context = Context::from_waker(futures::task::noop_waker_ref());
        match self.stream.as_mut().poll_next(&mut context) {
            Poll::Ready(Some(Ok(bytes))) => Some(bytes),
            Poll::Ready(None) => None,
            _ => panic!("in-memory source must produce a chunk or EOF"),
        }
    }

    fn assert_snapshot(&self, prompt: u64, completion: u64, semantics: &str) {
        let usage = snapshot_tapped_usage(&self.usage).expect("usage snapshot");
        assert_eq!(usage.prompt_tokens, prompt);
        assert_eq!(usage.completion_tokens, completion);
        assert_eq!(usage.total_tokens, prompt + completion);
        assert_eq!(
            snapshot_tapped_completion_semantics(&self.completion).as_deref(),
            Some(semantics)
        );
    }
}

fn frame(prompt: u64, completion: u64, reason: &str) -> Bytes {
    Bytes::from(format!(
        "data: {{\"usage\":{{\"prompt_tokens\":{prompt},\"completion_tokens\":{completion}}},\
         \"choices\":[{{\"finish_reason\":\"{reason}\"}}]}}\n\n"
    ))
}

fn assert_forwarded(actual: &Bytes, expected: &Bytes) {
    assert_eq!(actual.len(), expected.len());
    assert_eq!(
        actual.as_ptr(),
        expected.as_ptr(),
        "forward the original Bytes"
    );
}

#[test]
fn oversized_chunk_does_not_allocate_an_observer_copy() {
    let chunk = Bytes::from(vec![b'x'; FRAME_BYTES + 1024]);
    let mut observation = Observation::new(vec![chunk.clone()]);
    let (forwarded, largest) = allocation::largest_request(|| observation.next().unwrap());
    assert_forwarded(&forwarded, &chunk);
    assert!(
        largest <= 64 * 1024,
        "oversized chunk caused a {largest}-byte observer allocation"
    );
    assert!(snapshot_tapped_usage(&observation.usage).is_none());
    assert!(snapshot_tapped_completion_semantics(&observation.completion).is_none());
}

#[test]
fn cumulative_frame_limit_retains_previous_snapshots_and_recovers() {
    let first = frame(4, 5, "stop");
    let rejected = frame(900, 901, "length");
    let mut oversized = rejected[..rejected.len() - 2].to_vec();
    oversized.resize(FRAME_BYTES + 1, b' ');
    oversized.extend_from_slice(b"\n\n");
    let oversized = Bytes::from(oversized);
    let midpoint = oversized.len() / 2;
    let next = frame(7, 8, "tool_calls");
    let chunks = vec![
        first.clone(),
        oversized.slice(..midpoint),
        oversized.slice(midpoint..),
        next.clone(),
    ];
    let mut observation = Observation::new(chunks.clone());
    for chunk in &chunks[..3] {
        assert_forwarded(&observation.next().unwrap(), chunk);
        observation.assert_snapshot(4, 5, "stop");
    }
    assert_forwarded(&observation.next().unwrap(), &next);
    observation.assert_snapshot(7, 8, "tool_calls");
    assert!(observation.next().is_none());
}

#[test]
fn empty_data_line_pressure_is_bounded_and_later_frames_recover() {
    let mut excessive = Vec::new();
    for _ in 0..FRAME_LINES {
        excessive.extend_from_slice(b"data:\n");
    }
    excessive.extend_from_slice(&frame(900, 901, "length"));
    assert!(excessive.len() < FRAME_BYTES);
    let chunks = vec![
        frame(4, 5, "stop"),
        Bytes::from(excessive),
        frame(7, 8, "tool_calls"),
    ];
    let mut observation = Observation::new(chunks.clone());
    for chunk in &chunks[..2] {
        assert_forwarded(&observation.next().unwrap(), chunk);
        observation.assert_snapshot(4, 5, "stop");
    }
    assert_forwarded(&observation.next().unwrap(), &chunks[2]);
    observation.assert_snapshot(7, 8, "tool_calls");
}

#[test]
fn fragmented_utf8_crlf_and_invalid_lines_preserve_observation() {
    let payload = Bytes::from(format!(
        "data: {{\"text\":\"{}\",\"usage\":{{\"prompt_tokens\":3,\"completion_tokens\":2}},\
         \"choices\":[{{\"finish_reason\":\"stop\"}}]}}\r\n\r\n",
        '\u{00e9}'
    ));
    let split = payload.iter().position(|byte| *byte == 0xc3).unwrap() + 1;
    let chunks = vec![
        Bytes::from_static(b"\xff\n"),
        payload.slice(..split),
        Bytes::new(),
        payload.slice(split..payload.len() - 1),
        payload.slice(payload.len() - 1..),
    ];
    let mut observation = Observation::new(chunks.clone());
    for chunk in &chunks {
        assert_forwarded(&observation.next().unwrap(), chunk);
    }
    observation.assert_snapshot(3, 2, "stop");
}

#[test]
fn observer_specific_whitespace_and_carriage_return_rules_are_preserved() {
    let chunks = vec![
        Bytes::from_static(
            b"event:\tmessage_start \t\r\n\
              data: {\"message\":{\"usage\":{\"input_tokens\":3,\"output_tokens\":2}}}\r\n\r\r\n",
        ),
        Bytes::from_static(b"\n"),
        Bytes::from_static(
            b"event:\tmessage_delta\t\ndata: {\"delta\":{\"stop_reason\":\"tool_use\"}}\n\n",
        ),
        Bytes::from_static(
            b"event: message_delta\ndata: {\"delta\":{\"stop_reason\":\"tool_use\"}}\n\n",
        ),
    ];
    let mut observation = Observation::new(chunks.clone());
    assert_forwarded(&observation.next().unwrap(), &chunks[0]);
    assert!(snapshot_tapped_usage(&observation.usage).is_none());
    assert_forwarded(&observation.next().unwrap(), &chunks[1]);
    let usage = snapshot_tapped_usage(&observation.usage).unwrap();
    assert_eq!((usage.prompt_tokens, usage.completion_tokens), (3, 2));
    assert_forwarded(&observation.next().unwrap(), &chunks[2]);
    assert!(snapshot_tapped_completion_semantics(&observation.completion).is_none());
    assert_forwarded(&observation.next().unwrap(), &chunks[3]);
    observation.assert_snapshot(3, 2, "tool_calls");
}

#[test]
fn eof_does_not_flush_unterminated_lines_or_events() {
    let complete = frame(3, 2, "stop");
    for trim in [1, 2] {
        let chunk = complete.slice(..complete.len() - trim);
        let mut observation = Observation::new(vec![chunk.clone()]);
        assert_forwarded(&observation.next().unwrap(), &chunk);
        assert!(observation.next().is_none());
        assert!(snapshot_tapped_usage(&observation.usage).is_none());
        assert!(snapshot_tapped_completion_semantics(&observation.completion).is_none());
    }
}

#[test]
fn many_events_in_one_chunk_preserve_latest_metadata_and_byte_identity() {
    let mut chunk = Vec::new();
    for prompt in 1..=1024 {
        chunk.extend_from_slice(&frame(prompt, 1, "stop"));
    }
    let chunk = Bytes::from(chunk);
    let mut observation = Observation::new(vec![chunk.clone()]);
    assert_forwarded(&observation.next().unwrap(), &chunk);
    observation.assert_snapshot(1024, 1, "stop");
}
