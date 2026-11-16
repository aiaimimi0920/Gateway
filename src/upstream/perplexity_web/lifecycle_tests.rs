use super::{tests::*, *};
use bytes::Bytes;
use serde_json::json;
use std::sync::atomic::Ordering;

#[tokio::test]
async fn perplexity_web_total_deadline_covers_pending_body_and_releases_it() {
    let fixture = fixture(
        200,
        "text/event-stream",
        vec![Bytes::from_static(b": keepalive\n\n")],
        true,
        None,
    )
    .await;
    let started = std::time::Instant::now();
    let error = exchange(
        &client(),
        &session(),
        &request(),
        Duration::from_millis(150),
        &fixture.url,
    )
    .await
    .unwrap_err();
    assert_eq!(error.code.as_deref(), Some("perplexity_web_timeout"));
    assert!(started.elapsed() < Duration::from_secs(2));
    assert_eq!(fixture.seen.lock().unwrap().len(), 1);
    assert!(fixture.polled.load(Ordering::SeqCst));
    dropped(&fixture).await;
}

#[tokio::test]
async fn perplexity_web_cancellation_drops_body_without_a_background_retry_task() {
    let fixture = fixture(
        200,
        "text/event-stream",
        vec![Bytes::from_static(b": keepalive\n\n")],
        true,
        None,
    )
    .await;
    let (url, client, session, request) = (fixture.url.clone(), client(), session(), request());
    let task = tokio::spawn(async move {
        exchange(&client, &session, &request, Duration::from_secs(10), &url).await
    });
    tokio::time::timeout(Duration::from_secs(2), async {
        while !fixture.polled.load(Ordering::SeqCst) {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    dropped(&fixture).await;
    assert_eq!(fixture.seen.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn perplexity_web_unknown_length_body_limit_is_enforced_before_terminal() {
    let large_comment = Bytes::from(format!(":{}\n\n", "x".repeat(32 * 1024)));
    let fixture = fixture(
        200,
        "text/event-stream",
        vec![large_comment; 130],
        true,
        None,
    )
    .await;
    let error = exchange(
        &client(),
        &session(),
        &request(),
        Duration::from_secs(2),
        &fixture.url,
    )
    .await
    .unwrap_err();
    assert_eq!(error.code.as_deref(), Some("perplexity_web_response_limit"));
    dropped(&fixture).await;
}

#[tokio::test]
async fn perplexity_web_partial_eof_cannot_be_a_successful_http_200() {
    let partial = Bytes::from(format!(
        "data: {}\n\n",
        json!({"blocks":[{"intended_usage":"ask_text","markdown_block":{"answer":"partial"}}]})
    ));
    let fixture = fixture(200, "text/event-stream", vec![partial], false, None).await;
    let error = exchange(
        &client(),
        &session(),
        &request(),
        Duration::from_secs(2),
        &fixture.url,
    )
    .await
    .unwrap_err();
    assert_eq!(error.code.as_deref(), Some("perplexity_web_incomplete"));
    assert!(!error.retryable);
    assert_eq!(fixture.seen.lock().unwrap().len(), 1);
}
