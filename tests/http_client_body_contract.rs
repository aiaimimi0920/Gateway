//! Preserve the real catalog reader's incremental budget, error surface and cancellation.
use bytes::Bytes;
use futures::{Stream, StreamExt};
use neuro_gateway::protocol::chatgpt::codex_client::read_json;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

const LIMIT: usize = 2 * 1024 * 1024;

struct DropGuard(Arc<AtomicBool>);

impl Drop for DropGuard {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

fn response(
    stream: impl Stream<Item = Result<Bytes, std::io::Error>> + Send + 'static,
) -> rquest::Response {
    rquest::Response::from(axum::http::Response::new(rquest::Body::wrap_stream(stream)))
}

#[tokio::test]
async fn response_stream_io_error_retains_transport_classification_and_source() {
    let body = futures::stream::iter([Err::<Bytes, _>(std::io::Error::other(
        "fixture-body-failure",
    ))]);
    let stream = response(body).bytes_stream();
    futures::pin_mut!(stream);
    let error = stream.next().await.unwrap().unwrap_err();
    println!(
        "BODY_CLASSIFICATION decode={} request={} body={}",
        error.is_decode(),
        error.is_request(),
        error.is_body()
    );
    // rquest 为该重建响应报告 Decode，wreq 公开 HttpBody 路径报告 Request。
    // 两种都必须保留传输原因，且不能变成 connect/timeout 或 outgoing body 失败。
    assert!(matches!(
        (error.is_decode(), error.is_request()),
        (true, false) | (false, true)
    ));
    assert!(!error.is_body());
    assert!(!error.is_connect());
    assert!(!error.is_timeout());
    let classified = neuro_gateway::error::classify_network_error(&error, None);
    assert_eq!(classified.kind, neuro_gateway::error::ErrorKind::Unknown);
    assert!(!classified.retryable);
    let mut source: &dyn std::error::Error = &error;
    let mut found = false;
    loop {
        found |= source.to_string().contains("fixture-body-failure");
        match source.source() {
            Some(next) => source = next,
            None => break,
        }
    }
    assert!(found);
}

#[tokio::test]
async fn catalog_reader_accepts_exact_budget_across_chunk_boundaries() {
    let body = futures::stream::iter([
        Ok(Bytes::from_static(b"{\"ok\":")),
        Ok(Bytes::from(vec![b' '; LIMIT - b"{\"ok\":1}".len()])),
        Ok(Bytes::from_static(b"1}")),
    ]);
    assert_eq!(read_json(response(body)).await.unwrap()["ok"], 1);
}

#[tokio::test]
async fn catalog_overflow_stops_reading_and_releases_the_upstream() {
    let dropped = Arc::new(AtomicBool::new(false));
    let polls = Arc::new(AtomicUsize::new(0));
    let counter = polls.clone();
    let body = futures::stream::unfold((0, DropGuard(dropped.clone())), move |(index, guard)| {
        let counter = counter.clone();
        async move {
            counter.fetch_add(1, Ordering::SeqCst);
            let bytes = match index {
                0 => Bytes::from(vec![b' '; LIMIT]),
                1 => Bytes::from_static(b"x"),
                _ => panic!("reader must not poll beyond the oversized chunk"),
            };
            Some((Ok(bytes), (index + 1, guard)))
        }
    });
    let error = read_json(response(body)).await.unwrap_err();
    assert_eq!(error.message, "ChatGPT response too large");
    assert_eq!(polls.load(Ordering::SeqCst), 2);
    assert!(dropped.load(Ordering::SeqCst));
}

#[tokio::test]
async fn catalog_failures_preserve_fixed_secret_free_messages() {
    let body = futures::stream::iter([Err::<Bytes, _>(std::io::Error::other(
        "https://fixture.invalid/?token=must-not-leak",
    ))]);
    let error = read_json(response(body)).await.unwrap_err();
    assert_eq!(error.message, "ChatGPT response interrupted");
    let body = futures::stream::iter([Ok(Bytes::from_static(b"not-json-secret"))]);
    let error = read_json(response(body)).await.unwrap_err();
    assert_eq!(error.message, "ChatGPT returned invalid JSON");
}

#[tokio::test]
async fn catalog_cancellation_releases_a_pending_body() {
    let dropped = Arc::new(AtomicBool::new(false));
    let guard = DropGuard(dropped.clone());
    let body = futures::stream::once(async move {
        let _guard = guard;
        futures::future::pending::<Result<Bytes, std::io::Error>>().await
    });
    let mut read = Box::pin(read_json(response(body)));
    assert!(futures::poll!(read.as_mut()).is_pending());
    assert!(!dropped.load(Ordering::SeqCst));
    drop(read);
    assert!(dropped.load(Ordering::SeqCst));
}
