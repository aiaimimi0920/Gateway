use std::collections::VecDeque;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use bytes::Bytes;
use futures::{Stream, StreamExt};
use neuro_gateway::protocol::canonical::TokenUsage;
use neuro_gateway::upstream::stream::{
    snapshot_tapped_archive, snapshot_tapped_usage, tap_sse_usage, tap_stream_archive,
    StreamArchiveSnapshot, StreamMetrics, TrackedStream,
};
use serde_json::{json, Value};

async fn observe_usage(value: Value) -> TokenUsage {
    let wire = Bytes::from(format!("data: {}\n\n", json!({ "usage": value })));
    let (mut stream, usage) = tap_sse_usage(futures::stream::iter(vec![Ok(wire.clone())]));
    assert_eq!(stream.next().await.unwrap().unwrap(), wire);
    assert!(stream.next().await.is_none());
    snapshot_tapped_usage(&usage).expect("usage should remain observable")
}

async fn assert_large_usage(openai: bool, explicit_total: bool) {
    let mut value = if openai {
        json!({ "prompt_tokens": u64::MAX, "completion_tokens": 1 })
    } else {
        json!({ "input_tokens": u64::MAX, "output_tokens": 1 })
    };
    if explicit_total {
        value["total_tokens"] = json!(17);
    }
    let usage = observe_usage(value).await;
    assert_eq!(usage.prompt_tokens, u64::MAX);
    assert_eq!(usage.completion_tokens, 1);
    assert_eq!(usage.total_tokens, u64::MAX);
}

#[tokio::test]
async fn openai_usage_counter_overflow_saturates() {
    assert_large_usage(true, false).await;
}

#[tokio::test]
async fn openai_explicit_total_avoids_overflow_in_fallback_arithmetic() {
    assert_large_usage(true, true).await;
}

#[tokio::test]
async fn anthropic_usage_counter_overflow_saturates() {
    assert_large_usage(false, false).await;
}

#[tokio::test]
async fn anthropic_explicit_total_avoids_overflow_in_fallback_arithmetic() {
    assert_large_usage(false, true).await;
}

#[tokio::test]
async fn normal_usage_and_cache_details_keep_their_existing_merge_contract() {
    let usage = observe_usage(json!({
        "prompt_tokens": 8, "completion_tokens": 5, "total_tokens": 99,
        "cache_creation_input_tokens": 2, "prompt_tokens_details": { "cached_tokens": 3 }
    }))
    .await;
    assert_eq!(usage.prompt_tokens, 8);
    assert_eq!(usage.completion_tokens, 5);
    assert_eq!(usage.total_tokens, 13);
    assert_eq!(usage.cache_creation_input_tokens, Some(2));
    assert_eq!(usage.cache_read_input_tokens, Some(3));
}

async fn archive_parts(parts: &[&'static [u8]], limit: usize) -> StreamArchiveSnapshot {
    let chunks: Vec<_> = parts
        .iter()
        .map(|part| Ok::<_, rquest::Error>(Bytes::from_static(part)))
        .collect();
    let (stream, handle) = tap_stream_archive(futures::stream::iter(chunks), limit);
    let forwarded: Vec<_> = stream.collect::<Vec<_>>().await;
    let forwarded: Vec<_> = forwarded.into_iter().map(Result::unwrap).collect();
    let expected: Vec<_> = parts.iter().map(|part| Bytes::from_static(part)).collect();
    assert_eq!(forwarded, expected);
    snapshot_tapped_archive(&handle)
}

#[tokio::test]
async fn archive_empty_chunk_after_exact_capacity_keeps_complete_snapshot() {
    let snapshot = archive_parts(&[b"abc", b""], 3).await;
    assert_eq!(snapshot.text, "abc");
    assert!(!snapshot.truncated);
}

#[tokio::test]
async fn archive_zero_capacity_and_empty_chunks_do_not_report_data_loss() {
    let snapshot = archive_parts(&[b"", b""], 0).await;
    assert_eq!(snapshot.text, "");
    assert!(!snapshot.truncated);
}

#[tokio::test]
async fn archive_real_overflow_keeps_forwarding_and_retains_the_prefix() {
    let snapshot = archive_parts(&[b"ab", b"cd", b"", b"ef"], 3).await;
    assert_eq!(snapshot.text, "abc");
    assert!(snapshot.truncated);
}

#[tokio::test]
async fn archive_utf8_boundary_keeps_existing_lossy_snapshot_behavior() {
    let snapshot = archive_parts(&[b"\xe4\xb8\xad"], 2).await;
    assert_eq!(snapshot.text, "\u{fffd}");
    assert!(snapshot.truncated);
}

#[derive(Default)]
struct ProbeState {
    polls: AtomicUsize,
    drops: AtomicUsize,
    completions: Mutex<Vec<(StreamMetrics, bool, usize)>>,
}

struct ProbeStream {
    chunks: VecDeque<Result<Bytes, rquest::Error>>,
    state: Arc<ProbeState>,
}

impl Stream for ProbeStream {
    type Item = Result<Bytes, rquest::Error>;

    fn poll_next(mut self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.state.polls.fetch_add(1, Ordering::SeqCst);
        Poll::Ready(self.chunks.pop_front())
    }
}

impl Drop for ProbeStream {
    fn drop(&mut self) {
        self.state.drops.fetch_add(1, Ordering::SeqCst);
    }
}

fn tracked_probe(chunks: Vec<Result<Bytes, rquest::Error>>) -> (TrackedStream, Arc<ProbeState>) {
    let state = Arc::new(ProbeState::default());
    let observed = Arc::clone(&state);
    let stream = TrackedStream::new(
        ProbeStream {
            chunks: chunks.into(),
            state: Arc::clone(&state),
        },
        move |metrics, success| {
            let dropped = observed.drops.load(Ordering::SeqCst);
            observed
                .completions
                .lock()
                .unwrap()
                .push((metrics, success, dropped));
        },
    );
    (stream, state)
}

fn assert_completion(state: &ProbeState, success: bool, chunks: u64, bytes: u64) {
    assert_eq!(state.drops.load(Ordering::SeqCst), 1);
    let events = state.completions.lock().unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].0.chunk_count, chunks);
    assert_eq!(events[0].0.total_bytes, bytes);
    assert_eq!(events[0].1, success);
    assert_eq!(
        events[0].2, 1,
        "release upstream before invoking completion"
    );
}

#[tokio::test]
async fn tracked_clean_end_releases_upstream_once_and_stops_polling() {
    let (mut stream, state) = tracked_probe(vec![Ok(Bytes::new()), Ok(Bytes::from_static(b"abc"))]);
    assert!(stream.next().await.unwrap().unwrap().is_empty());
    assert_eq!(
        stream.next().await.unwrap().unwrap(),
        Bytes::from_static(b"abc")
    );
    assert!(stream.next().await.is_none());
    assert_completion(&state, true, 2, 3);
    assert!(stream.next().await.is_none());
    assert_eq!(state.polls.load(Ordering::SeqCst), 3);
    drop(stream);
    assert_completion(&state, true, 2, 3);
}

#[tokio::test]
async fn tracked_error_releases_upstream_once_and_prevents_later_items() {
    let error = rquest::Error::from(serde_json::Error::io(std::io::Error::new(
        std::io::ErrorKind::UnexpectedEof,
        "synthetic upstream failure",
    )));
    let (mut stream, state) = tracked_probe(vec![Err(error), Ok(Bytes::from_static(b"late"))]);
    assert!(stream.next().await.unwrap().is_err());
    assert_completion(&state, false, 0, 0);
    assert!(stream.next().await.is_none());
    assert_eq!(state.polls.load(Ordering::SeqCst), 1);
    drop(stream);
    assert_completion(&state, false, 0, 0);
}

#[tokio::test]
async fn tracked_early_drop_releases_upstream_before_failure_callback() {
    let (mut stream, state) = tracked_probe(vec![
        Ok(Bytes::from_static(b"a")),
        Ok(Bytes::from_static(b"b")),
    ]);
    assert_eq!(
        stream.next().await.unwrap().unwrap(),
        Bytes::from_static(b"a")
    );
    drop(stream);
    assert_completion(&state, false, 1, 1);
    assert_eq!(state.polls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn tracked_completion_retains_usage_and_archive_snapshots() {
    let wire =
        Bytes::from_static(b"data: {\"usage\":{\"prompt_tokens\":2,\"completion_tokens\":3}}\n\n");
    let source = futures::stream::iter(vec![Ok(wire.clone())]);
    let (usage_stream, usage) = tap_sse_usage(source);
    let (archive_stream, archive) = tap_stream_archive(usage_stream, 1024);
    let observed = Arc::new(Mutex::new(None));
    let completion = Arc::clone(&observed);
    let mut tracked = TrackedStream::new(archive_stream, move |_, success| {
        assert!(success);
        *completion.lock().unwrap() = Some((
            snapshot_tapped_usage(&usage).unwrap(),
            snapshot_tapped_archive(&archive),
        ));
    });
    assert_eq!(tracked.next().await.unwrap().unwrap(), wire);
    assert!(tracked.next().await.is_none());
    let snapshot = observed.lock().unwrap();
    let (usage, archive) = snapshot.as_ref().unwrap();
    assert_eq!(usage.total_tokens, 5);
    assert_eq!(archive.text.as_bytes(), wire.as_ref());
    assert!(!archive.truncated);
}
