//! A Responses client may stop reading immediately after its terminal event.
use super::*;
use futures::{stream, StreamExt};
use std::sync::{Arc, Mutex};

#[tokio::test]
async fn completed_response_then_client_close_is_success() {
    let bytes = Bytes::from_static(
        b"event: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{\"status\":\"completed\",\"usage\":{\"input_tokens\":1,\"output_tokens\":1}}}\n\n",
    );
    let result = Arc::new(Mutex::new(None));
    let recorded = Arc::clone(&result);
    let pending = stream::iter([Ok::<_, std::io::Error>(bytes.clone())]).chain(stream::pending());
    let mut tracked = TrackedStream::new_with_error(pending, move |metrics, success| {
        *recorded.lock().unwrap() = Some((metrics.termination, success));
    })
    .with_responses_terminal();
    assert_eq!(tracked.next().await.unwrap().unwrap(), bytes);
    drop(tracked);
    assert_eq!(
        *result.lock().unwrap(),
        Some((Some(StreamTermination::Completed), true))
    );
}

#[tokio::test]
async fn cancellation_without_terminal_keeps_existing_failure_semantics() {
    let result = Arc::new(Mutex::new(None));
    let recorded = Arc::clone(&result);
    let mut tracked = TrackedStream::new_with_error(
        stream::iter([Ok::<_, std::io::Error>(Bytes::from_static(
            b"data: {\"type\":\"response.output_text.done\",\"text\":\"OK\"}\n\n",
        ))])
        .chain(stream::pending()),
        move |metrics, success| *recorded.lock().unwrap() = Some((metrics.termination, success)),
    )
    .with_responses_terminal();
    tracked.next().await.unwrap().unwrap();
    drop(tracked);
    assert_eq!(
        *result.lock().unwrap(),
        Some((Some(StreamTermination::Cancelled), false))
    );
}

#[tokio::test]
async fn transport_error_after_terminal_is_not_success_and_callback_fires_once() {
    let result = Arc::new(Mutex::new(Vec::new()));
    let recorded = Arc::clone(&result);
    let mut tracked = TrackedStream::new_with_error(
        stream::iter([
            Ok(Bytes::from_static(b"data: {\"type\":\"response.completed\",\"response\":{\"status\":\"completed\"}}\n\n")),
            Err(std::io::Error::other("fixture")),
        ]),
        move |metrics, success| recorded.lock().unwrap().push((metrics.termination, success)),
    ).with_responses_terminal();
    tracked.next().await.unwrap().unwrap();
    assert!(tracked.next().await.unwrap().is_err());
    drop(tracked);
    assert_eq!(
        *result.lock().unwrap(),
        [(Some(StreamTermination::Interrupted), false)]
    );
}

#[tokio::test]
async fn non_responses_streams_do_not_opt_in_to_terminal_interpretation() {
    let result = Arc::new(Mutex::new(None));
    let recorded = Arc::clone(&result);
    let mut tracked = TrackedStream::new_with_error(
        stream::iter([Ok::<_, std::io::Error>(Bytes::from_static(
            b"data: {\"type\":\"response.completed\",\"response\":{\"status\":\"completed\"}}\n\n",
        ))])
        .chain(stream::pending()),
        move |metrics, success| *recorded.lock().unwrap() = Some((metrics.termination, success)),
    );
    tracked.next().await.unwrap().unwrap();
    drop(tracked);
    assert_eq!(
        *result.lock().unwrap(),
        Some((Some(StreamTermination::Cancelled), false))
    );
}
