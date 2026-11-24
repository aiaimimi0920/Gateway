//! Exercises the real multipart parser and producer/consumer cancellation without allocating the file.
use super::*;
use axum::{
    body::Body,
    extract::{FromRequest, Request},
};
use bytes::Bytes;
use futures::TryStreamExt;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use tower::ServiceExt;

async fn form(
    size: usize,
    tail: &'static [u8],
    count: Arc<AtomicUsize>,
) -> (Multipart, super::super::ingress::Budget) {
    let head = futures::stream::once(async {
        Ok::<_, std::io::Error>(Bytes::from_static(
            b"--b\r\nContent-Disposition: form-data; name=\"file\"\r\n\r\n",
        ))
    });
    let data = futures::stream::unfold(size, move |remaining| {
        let count = count.clone();
        async move {
            if remaining == 0 {
                return None;
            }
            let n = remaining.min(64 * 1024);
            count.fetch_add(1, Ordering::SeqCst);
            Some((
                Ok::<_, std::io::Error>(Bytes::from(vec![7; n])),
                remaining - n,
            ))
        }
    });
    let tail =
        futures::stream::once(async move { Ok::<_, std::io::Error>(Bytes::from_static(tail)) });
    let request = Request::builder()
        .header("content-type", "multipart/form-data; boundary=b")
        .body(Body::from_stream(head.chain(data).chain(tail)))
        .unwrap();
    // Unit tests bypass only Axum's default 2 MiB limit, as the dedicated management route does.
    let (request, budget) = super::super::ingress::bounded(request);
    let multipart = tower::ServiceBuilder::new()
        .layer(axum::extract::DefaultBodyLimit::disable())
        .service(tower::service_fn(|request| async move {
            Ok::<_, std::convert::Infallible>(Multipart::from_request(request, &()).await.unwrap())
        }))
        .oneshot(request)
        .await
        .unwrap();
    (multipart, budget)
}

async fn consume(upload: UploadBody) -> Result<usize, GatewayError> {
    let response = rquest::Response::from(axum::http::Response::new(upload.body));
    let reader = async {
        response
            .bytes_stream()
            .try_fold(0usize, |n, bytes| async move { Ok(n + bytes.len()) })
            .await
            .map_err(|_| GatewayError::bad_request("consumer failed"))
    };
    tokio::try_join!(reader, upload.completion).map(|(size, ())| size)
}

#[tokio::test]
async fn files_streaming_accepts_large_exact_body_and_rejects_malformed_end() {
    let count = Arc::new(AtomicUsize::new(0));
    let size = 64 * 1024 * 1024 + 1;
    let (multipart, budget) = form(size, b"\r\n--b--\r\n", count.clone()).await;
    assert_eq!(consume(body(multipart, size, budget)).await.unwrap(), size);
    for (actual, declared, tail) in [
        (2, 3, b"\r\n--b--\r\n".as_slice()),
        (4, 3, b"\r\n--b--\r\n".as_slice()),
        (
            3,
            3,
            b"\r\n--b\r\nContent-Disposition: form-data; name=\"extra\"\r\n\r\nx\r\n--b--\r\n"
                .as_slice(),
        ),
        (3, 3, b"".as_slice()),
    ] {
        let (multipart, budget) = form(actual, tail, count.clone()).await;
        assert!(consume(body(multipart, declared, budget)).await.is_err());
    }
}

#[tokio::test]
async fn files_streaming_backpressure_and_drop_stop_reading_without_a_detached_task() {
    let count = Arc::new(AtomicUsize::new(0));
    let (multipart, budget) = form(5 * 1024 * 1024 * 1024, b"\r\n--b--\r\n", count.clone()).await;
    let upload = body(multipart, 5 * 1024 * 1024 * 1024, budget);
    let mut producer = upload.completion;
    for _ in 0..16 {
        assert!(futures::poll!(&mut producer).is_pending());
        tokio::task::yield_now().await;
    }
    let read = count.load(Ordering::SeqCst);
    assert!(
        (1..=4).contains(&read),
        "read {read} chunks without a consumer"
    );
    drop(producer);
    drop(upload.body);
    tokio::task::yield_now().await;
    assert_eq!(count.load(Ordering::SeqCst), read);
}
