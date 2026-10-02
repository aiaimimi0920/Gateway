//! 新错误类型经过完整观察链时，快照、对象身份和终止时序仍可验证。
use super::*;
use crate::protocol::stream_error::{ProtocolStreamError, StreamError};
use crate::protocol::stream_error_test_support::probe;
use futures::StreamExt;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

type Error = StreamError<std::io::Error>;

const WIRE: &[u8] = b"data: {\"choices\":[{\"finish_reason\":\"stop\"}],\"usage\":{\"prompt_tokens\":2,\"completion_tokens\":3}}\n\n";

#[tokio::test]
async fn typed_eof_releases_before_callback_and_preserves_all_snapshots() {
    let (source, state) =
        probe::<Error>(vec![Ok(Bytes::new()), Ok(Bytes::from_static(WIRE))], false);
    let (stream, usage) = tap_sse_usage_with_error(source);
    let (stream, completion) = tap_sse_completion_semantics_with_error(stream);
    let (stream, archive) = tap_stream_archive_with_error(stream, WIRE.len());
    let observed = Arc::new(Mutex::new(Vec::new()));
    let output = Arc::clone(&observed);
    let probe_state = Arc::clone(&state);
    let started = Instant::now() - std::time::Duration::from_millis(40);
    let mut tracked =
        TrackedStream::new_with_started_at_and_error(stream, started, move |metrics, success| {
            assert_eq!(probe_state.drops.load(Ordering::SeqCst), 1);
            output.lock().unwrap().push((
                metrics,
                success,
                snapshot_tapped_usage(&usage),
                snapshot_tapped_completion_semantics(&completion),
                snapshot_tapped_archive(&archive),
            ));
        });
    assert!(tracked.next().await.unwrap().unwrap().is_empty());
    assert_eq!(tracked.next().await.unwrap().unwrap().as_ref(), WIRE);
    assert!(tracked.next().await.is_none());
    assert!(tracked.next().await.is_none());
    drop(tracked);
    let events = observed.lock().unwrap();
    assert_eq!(events.len(), 1);
    let (metrics, success, usage, completion, archive) = &events[0];
    assert!(*success);
    assert_eq!(metrics.chunk_count, 2);
    assert_eq!(metrics.total_bytes, WIRE.len() as u64);
    assert!(metrics.first_token_latency_ms.unwrap() >= 40);
    assert_eq!(usage.as_ref().unwrap().total_tokens, 5);
    assert_eq!(completion.as_deref(), Some("stop"));
    assert_eq!(archive.text.as_bytes(), WIRE);
    assert!(!archive.truncated);
    assert_eq!(state.polls.load(Ordering::SeqCst), 3);
}

#[derive(Debug)]
struct Marker(u64);
impl std::fmt::Display for Marker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "transport marker {}", self.0)
    }
}
impl std::error::Error for Marker {}

#[tokio::test]
async fn both_error_variants_survive_observers_without_late_poll_or_success() {
    let marker = Box::new(Marker(42));
    let identity = (&*marker as *const Marker) as usize;
    // 显式转成错误 trait object，避免 Error::new 再包一层 Box<Marker>。
    let marker: Box<dyn std::error::Error + Send + Sync> = marker;
    let errors = [
        Error::Transport(std::io::Error::new(std::io::ErrorKind::BrokenPipe, marker)),
        Error::Protocol(ProtocolStreamError::invalid_data("protocol marker")),
    ];
    for (index, error) in errors.into_iter().enumerate() {
        let expected = error.to_string();
        let (source, state) = probe(
            vec![
                Ok(Bytes::from_static(WIRE)),
                Err(error),
                Ok(Bytes::from_static(b"late")),
            ],
            false,
        );
        let (stream, usage) = tap_sse_usage_with_error(source);
        let (stream, completion) = tap_sse_completion_semantics_with_error(stream);
        let (stream, archive) = tap_stream_archive_with_error(stream, 7);
        let calls = Arc::new(AtomicUsize::new(0));
        let observed = Arc::clone(&calls);
        let probe_state = Arc::clone(&state);
        let mut tracked = TrackedStream::new_with_error(stream, move |metrics, success| {
            assert!(!success);
            assert_eq!(metrics.chunk_count, 1);
            assert_eq!(probe_state.drops.load(Ordering::SeqCst), 1);
            observed.fetch_add(1, Ordering::SeqCst);
        });
        assert_eq!(tracked.next().await.unwrap().unwrap().as_ref(), WIRE);
        let error = tracked.next().await.unwrap().unwrap_err();
        assert_eq!(error.to_string(), expected);
        match error {
            Error::Transport(error) => {
                assert_eq!(index, 0);
                let marker = error.get_ref().unwrap().downcast_ref::<Marker>().unwrap();
                assert_eq!((marker as *const Marker) as usize, identity);
            }
            Error::Protocol(_) => assert_eq!(index, 1),
        }
        assert!(tracked.next().await.is_none());
        assert_eq!(snapshot_tapped_usage(&usage).unwrap().total_tokens, 5);
        assert_eq!(
            snapshot_tapped_completion_semantics(&completion).as_deref(),
            Some("stop")
        );
        let archive = snapshot_tapped_archive(&archive);
        assert_eq!(archive.text.as_bytes(), &WIRE[..7]);
        assert!(archive.truncated);
        drop(tracked);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(state.polls.load(Ordering::SeqCst), 2);
        assert_eq!(state.drops.load(Ordering::SeqCst), 1);
    }
}

#[tokio::test]
async fn cancelling_pending_observation_chain_reports_failure_once() {
    let (source, state) = probe::<Error>(vec![], true);
    let (stream, _) = tap_sse_usage_with_error(source);
    let (stream, _) = tap_sse_completion_semantics_with_error(stream);
    let (stream, _) = tap_stream_archive_with_error(stream, 16);
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = Arc::clone(&calls);
    let probe_state = Arc::clone(&state);
    let mut tracked = TrackedStream::new_with_error(stream, move |metrics, success| {
        assert!(!success);
        assert_eq!(metrics.chunk_count, 0);
        assert_eq!(probe_state.drops.load(Ordering::SeqCst), 1);
        observed.fetch_add(1, Ordering::SeqCst);
    });
    assert!(futures::poll!(tracked.next()).is_pending());
    drop(tracked);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(state.polls.load(Ordering::SeqCst), 1);
}
