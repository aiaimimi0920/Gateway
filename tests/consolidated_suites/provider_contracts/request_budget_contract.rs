//! Virtual time proves shared limits, cancellation and post-handoff no-replay.
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;

use bytes::Bytes;
use futures::{stream, Stream, StreamExt};
use neuro_gateway::error::{FallbackHint, GatewayError};
use neuro_gateway::pipeline::request_budget::RequestBudget;
use neuro_gateway::protocol::stream_error::StreamError;
use tokio::time::{advance, timeout};

#[tokio::test(start_paused = true)]
async fn clones_share_slots_and_keep_the_complete_last_failure() {
    let budget = RequestBudget::new(2, Duration::from_secs(10));
    let recovery = budget.clone();
    budget.begin_attempt().unwrap();
    let original = GatewayError::service_unavailable("last real failure")
        .with_code("fixture_original_code")
        .with_provider("fixture_provider");
    budget.observe_error(&original);
    recovery.begin_attempt().unwrap();
    let error = budget.begin_attempt().unwrap_err();
    assert_eq!(budget.attempts(), 2);
    assert_eq!(error.code, original.code);
    assert_eq!(error.request_budget_stop_reason(), Some("attempt_limit"));
    assert_eq!(error.message, original.message);
    assert_eq!(error.kind, original.kind);
    assert_eq!(error.http_status, original.http_status);
    assert_eq!(error.provider_name, original.provider_name);
    assert!(!error.retryable);
    assert!(matches!(error.fallback_hint, FallbackHint::Abort { .. }));
    assert_eq!(recovery.last_upstream_error().unwrap().code, original.code);
    budget.observe_error(&error);
    assert_eq!(budget.last_upstream_error().unwrap().code, original.code);
}

#[tokio::test(start_paused = true)]
async fn expired_deadline_never_polls_a_new_send_or_consumes_a_slot() {
    let budget = RequestBudget::new(6, Duration::from_secs(1));
    advance(Duration::from_secs(1)).await;
    let polls = AtomicUsize::new(0);
    let error = budget
        .run(async {
            polls.fetch_add(1, Ordering::Relaxed);
            Ok(())
        })
        .await
        .unwrap_err();
    assert_eq!(error.code.as_deref(), Some("budget_exhausted"));
    assert_eq!(polls.load(Ordering::Relaxed), 0);
    assert_eq!(budget.attempts(), 0);
}

#[tokio::test(start_paused = true)]
async fn pending_operation_stops_at_the_original_deadline() {
    let budget = RequestBudget::new(6, Duration::from_secs(2));
    budget.begin_attempt().unwrap();
    let error = budget
        .run(std::future::pending::<Result<(), GatewayError>>())
        .await
        .unwrap_err();
    assert_eq!(error.code.as_deref(), Some("budget_exhausted"));
    assert!(
        matches!(error.fallback_hint, FallbackHint::Abort { reason } if reason == "budget_exhausted:deadline")
    );
    assert_eq!(budget.attempts(), 1);
    assert!(budget.clone().begin_attempt().is_err());
}

#[tokio::test(start_paused = true)]
async fn dropping_dispatch_persists_cancellation_for_recovery_clones() {
    let budget = RequestBudget::new(6, Duration::from_secs(10));
    {
        let pending = budget.run(std::future::pending::<Result<(), GatewayError>>());
        tokio::pin!(pending);
        assert!(timeout(Duration::from_millis(1), &mut pending)
            .await
            .is_err());
    }
    let error = budget.clone().begin_attempt().unwrap_err();
    assert!(
        matches!(error.fallback_hint, FallbackHint::Abort { reason } if reason == "budget_exhausted:cancelled")
    );
    assert_eq!(budget.attempts(), 0);
}

struct PendingAfterChunk {
    chunk: Option<Bytes>,
    dropped: Arc<AtomicUsize>,
}
impl Stream for PendingAfterChunk {
    type Item = Result<Bytes, StreamError<std::io::Error>>;
    fn poll_next(mut self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match self.chunk.take() {
            Some(chunk) => Poll::Ready(Some(Ok(chunk))),
            None => Poll::Pending,
        }
    }
}
impl Drop for PendingAfterChunk {
    fn drop(&mut self) {
        self.dropped.fetch_add(1, Ordering::Relaxed);
    }
}

#[tokio::test(start_paused = true)]
async fn handed_off_stream_keeps_bytes_then_times_out_once_without_replay() {
    let budget = RequestBudget::new(6, Duration::from_secs(1));
    budget.begin_attempt().unwrap();
    let dropped = Arc::new(AtomicUsize::new(0));
    let source = PendingAfterChunk {
        chunk: Some(Bytes::from_static(b"original-text/tool-call")),
        dropped: dropped.clone(),
    };
    let mut stream = budget.constrain_stream(source);
    assert_eq!(
        stream.next().await.unwrap().unwrap(),
        "original-text/tool-call"
    );
    assert!(budget.clone().begin_attempt().is_err());
    advance(Duration::from_secs(1)).await;
    let error = stream.next().await.unwrap().unwrap_err();
    assert!(error.to_string().contains("budget_exhausted:deadline"));
    assert_eq!(dropped.load(Ordering::Relaxed), 1);
    assert!(stream.next().await.is_none());
    assert_eq!(budget.attempts(), 1);
}

#[tokio::test(start_paused = true)]
async fn dropping_handed_off_stream_releases_transport_and_blocks_future_sends() {
    let budget = RequestBudget::new(6, Duration::from_secs(10));
    let dropped = Arc::new(AtomicUsize::new(0));
    drop(budget.constrain_stream(PendingAfterChunk {
        chunk: None,
        dropped: dropped.clone(),
    }));
    assert_eq!(dropped.load(Ordering::Relaxed), 1);
    assert!(budget.begin_attempt().is_err());
}

#[tokio::test(start_paused = true)]
async fn stream_eof_and_existing_errors_do_not_create_deadline_errors() {
    let budget = RequestBudget::new(6, Duration::from_secs(10));
    let original = StreamError::Transport(std::io::Error::other("original transport error"));
    let mut stream = budget.constrain_stream(stream::iter(vec![Ok(Bytes::new()), Err(original)]));
    assert!(stream.next().await.unwrap().unwrap().is_empty());
    assert_eq!(
        stream.next().await.unwrap().unwrap_err().to_string(),
        "original transport error"
    );
    assert!(stream.next().await.is_none());
}

#[tokio::test(start_paused = true)]
async fn deadline_cause_is_visible_before_a_pending_future_drops_its_stream() {
    let budget = RequestBudget::new(6, Duration::from_secs(1));
    let dropped = Arc::new(AtomicUsize::new(0));
    let result: Result<(), GatewayError> = budget
        .run(async {
            let _stream = budget.constrain_stream(PendingAfterChunk {
                chunk: None,
                dropped: dropped.clone(),
            });
            std::future::pending().await
        })
        .await;
    assert_eq!(
        result.unwrap_err().request_budget_stop_reason(),
        Some("deadline")
    );
    assert_eq!(budget.stopped_reason(), Some("deadline"));
    assert_eq!(dropped.load(Ordering::Relaxed), 1);
}

#[tokio::test]
async fn late_non_yielding_success_is_discarded_before_handoff() {
    let budget = RequestBudget::new(6, Duration::from_millis(5));
    let dropped = Arc::new(AtomicUsize::new(0));
    let result = budget
        .run(async {
            let stream = budget.constrain_stream(PendingAfterChunk {
                chunk: None,
                dropped: dropped.clone(),
            });
            // Simulate an adapter poll overrunning the timer without yielding.
            std::thread::sleep(Duration::from_millis(15));
            Ok(stream)
        })
        .await;
    assert_eq!(
        result.err().unwrap().request_budget_stop_reason(),
        Some("deadline")
    );
    assert_eq!(budget.stopped_reason(), Some("deadline"));
    assert_eq!(dropped.load(Ordering::Relaxed), 1);
}

#[tokio::test]
async fn successful_buffered_dispatch_cannot_be_reentered() {
    let budget = RequestBudget::new(6, Duration::from_secs(30));
    budget.begin_attempt().unwrap();
    assert_eq!(budget.run(async { Ok(7) }).await.unwrap(), 7);
    assert_eq!(
        budget
            .begin_attempt()
            .unwrap_err()
            .request_budget_stop_reason(),
        Some("response_handed_off")
    );
    assert_eq!(budget.attempts(), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn simultaneous_send_starts_cannot_overbook_shared_slots() {
    let budget = RequestBudget::new(6, Duration::from_secs(30));
    let barrier = Arc::new(tokio::sync::Barrier::new(32));
    let mut tasks = Vec::new();
    for _ in 0..32 {
        let (budget, barrier) = (budget.clone(), barrier.clone());
        tasks.push(tokio::spawn(async move {
            barrier.wait().await;
            budget.begin_attempt().is_ok()
        }));
    }
    let mut admitted = 0;
    for task in tasks {
        admitted += usize::from(task.await.unwrap());
    }
    assert_eq!(admitted, 6);
    assert_eq!(budget.attempts(), 6);
}

#[tokio::test]
async fn budget_http_marker_preserves_upstream_body_and_never_exposes_free_text_hint() {
    use axum::response::IntoResponse;
    let budget = RequestBudget::new(1, Duration::from_secs(30));
    let original =
        GatewayError::rate_limited("fixture original error", 5000).with_code("fixture_code");
    budget.observe_error(&original);
    budget.begin_attempt().unwrap();
    let response = budget.begin_attempt().unwrap_err().into_response();
    assert_eq!(response.status(), 429);
    assert_eq!(
        response.headers()["x-gateway-error-code"],
        "budget_exhausted"
    );
    assert_eq!(response.headers()["x-gateway-stop-reason"], "attempt_limit");
    assert!(!response.headers().contains_key("retry-after"));
    let bytes = axum::body::to_bytes(response.into_body(), 1024)
        .await
        .unwrap();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["error"]["code"], "fixture_code");
    assert_eq!(body["error"]["message"], "fixture original error");
    assert_eq!(body["error"]["type"], "RateLimit");
    let mut ordinary = GatewayError::bad_request("fixture input");
    ordinary.fallback_hint = FallbackHint::Abort {
        reason: "budget_exhausted:untrusted-user-string".to_string(),
    };
    assert!(!ordinary
        .into_response()
        .headers()
        .contains_key("x-gateway-error-code"));
}
