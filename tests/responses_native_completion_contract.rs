//! Native Responses validation must retain byte ownership and transport/cancellation semantics.
use bytes::Bytes;
use futures::{stream, Stream, StreamExt};
use neuro_gateway::protocol::responses::require_responses_completion;
use neuro_gateway::protocol::stream_error::StreamError;
use neuro_gateway::upstream::stream::{StreamTermination, TrackedStream};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::task::Poll;

type Error = StreamError<std::io::Error>;
const COMPLETE: &[u8] = b": heartbeat\r\nevent: response.completed\r\ndata: {\"type\":\"response.completed\",\"response\":{\"status\":\"completed\",\"output\":[],\"unknown\":42}}\r\n\r\ndata: [DONE]\r\n\r\n";
const PARTIAL: &[u8] = b"data: {\"type\":\"response.output_text.delta\",\"delta\":\"partial\"}\n\n";

struct DropProbe(Arc<AtomicUsize>);
impl Drop for DropProbe {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

fn probe(
    items: Vec<Result<Bytes, Error>>,
    pending: bool,
    drops: Arc<AtomicUsize>,
) -> impl Stream<Item = Result<Bytes, Error>> + Send + 'static {
    let guard = DropProbe(drops);
    let mut items = VecDeque::from(items);
    stream::poll_fn(move |_| {
        let _keep_alive = &guard;
        match items.pop_front() {
            Some(item) => Poll::Ready(Some(item)),
            None if pending => Poll::Pending,
            None => Poll::Ready(None),
        }
    })
}

#[tokio::test]
async fn native_completion_preserves_every_chunk_allocation_and_byte_split() {
    for split in 0..=COMPLETE.len() {
        let chunks = vec![
            Bytes::copy_from_slice(&COMPLETE[..split]),
            Bytes::copy_from_slice(&COMPLETE[split..]),
        ];
        let pointers: Vec<_> = chunks.iter().map(|chunk| chunk.as_ptr()).collect();
        let chunks = chunks.into_iter().map(Ok::<_, Error>);
        let received = require_responses_completion(stream::iter(chunks))
            .collect::<Vec<_>>()
            .await;
        assert_eq!(received.len(), 2);
        let received: Vec<_> = received.into_iter().map(Result::unwrap).collect();
        assert_eq!(
            received
                .iter()
                .map(|chunk| chunk.as_ptr())
                .collect::<Vec<_>>(),
            pointers
        );
        assert_eq!(received.concat(), COMPLETE);
    }
}

#[tokio::test]
async fn missing_terminal_is_one_error_and_releases_source_before_callback() {
    for wire in [PARTIAL, b"data: [DONE]\n\n".as_slice(), b"".as_slice()] {
        let drops = Arc::new(AtomicUsize::new(0));
        let observed = Arc::new(Mutex::new(Vec::new()));
        let source = probe(
            vec![Ok(Bytes::copy_from_slice(wire))],
            false,
            Arc::clone(&drops),
        );
        let capture = Arc::clone(&observed);
        let capture_drops = Arc::clone(&drops);
        let result = TrackedStream::new_with_error(
            require_responses_completion(source),
            move |metrics, success| {
                assert_eq!(capture_drops.load(Ordering::SeqCst), 1);
                capture.lock().unwrap().push((metrics.termination, success));
            },
        )
        .collect::<Vec<_>>()
        .await;
        let errors: Vec<_> = result
            .iter()
            .filter_map(|item| item.as_ref().err())
            .collect();
        assert_eq!(errors.len(), 1);
        assert!(matches!(errors[0], Error::Protocol(_)));
        assert!(errors[0]
            .to_string()
            .contains("responses_stream_incomplete"));
        assert_eq!(
            &*observed.lock().unwrap(),
            &[(Some(StreamTermination::Interrupted), false)]
        );
    }
}

#[tokio::test]
async fn original_transport_error_is_not_replaced_by_incomplete() {
    let drops = Arc::new(AtomicUsize::new(0));
    let original = std::io::Error::new(std::io::ErrorKind::ConnectionReset, "transport marker");
    let source = probe(
        vec![
            Ok(Bytes::from_static(PARTIAL)),
            Err(Error::Transport(original)),
        ],
        false,
        Arc::clone(&drops),
    );
    let result = require_responses_completion(source)
        .collect::<Vec<_>>()
        .await;
    assert_eq!(result.len(), 2);
    assert!(
        matches!(&result[1], Err(Error::Transport(error)) if error.kind() == std::io::ErrorKind::ConnectionReset && error.to_string() == "transport marker")
    );
    assert_eq!(drops.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn dropping_pending_native_stream_is_cancellation_not_truncation() {
    let drops = Arc::new(AtomicUsize::new(0));
    let observed = Arc::new(Mutex::new(Vec::new()));
    let source = probe(
        vec![Ok(Bytes::from_static(PARTIAL))],
        true,
        Arc::clone(&drops),
    );
    let capture = Arc::clone(&observed);
    let mut tracked = TrackedStream::new_with_error(
        require_responses_completion(source),
        move |metrics, success| {
            capture.lock().unwrap().push((metrics.termination, success));
        },
    );
    assert_eq!(tracked.next().await.unwrap().unwrap().as_ref(), PARTIAL);
    assert!(futures::poll!(tracked.next()).is_pending());
    assert!(observed.lock().unwrap().is_empty());
    drop(tracked);
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    assert_eq!(
        &*observed.lock().unwrap(),
        &[(Some(StreamTermination::Cancelled), false)]
    );
}

#[tokio::test]
async fn native_incomplete_is_valid_but_failure_or_invalid_json_is_not() {
    let incomplete = String::from_utf8(COMPLETE.to_vec())
        .unwrap()
        .replace("completed", "incomplete");
    assert!(
        require_responses_completion(stream::iter(vec![Ok::<_, Error>(Bytes::from(incomplete))]))
            .collect::<Vec<_>>()
            .await
            .iter()
            .all(Result::is_ok)
    );
    for wire in [
        "event: response.failed\ndata: {\"type\":\"response.failed\",\"message\":\"private-marker\"}\n\n",
        "event: response.completed\ndata: {invalid}\n\n",
        "event: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{\"status\":\"failed\"}}\n\n",
    ] {
        let items = require_responses_completion(stream::iter(vec![Ok::<_, Error>(Bytes::copy_from_slice(wire.as_bytes()))]))
            .collect::<Vec<_>>().await;
        assert_eq!(items.len(), 1);
        let error = items[0].as_ref().unwrap_err();
        assert!(matches!(error, Error::Protocol(_)));
        assert!(!error.to_string().contains("private-marker"));
    }
}
