use super::{
    collect_bounded_upstream_charset_text_with_provider, MAX_ACCUMULATED_UPSTREAM_BODY_BYTES,
};
use crate::error::classify_network_error;
use bytes::Bytes;
use rquest::header::{HeaderValue, CONTENT_TYPE};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

const PROVIDER: &str = "gemini_canvas_compatible";
const LABEL: &str = "charset fixture";

fn response(body: impl Into<rquest::Body>, content_type: Option<HeaderValue>) -> rquest::Response {
    let mut response = axum::http::Response::new(body.into());
    if let Some(content_type) = content_type {
        response.headers_mut().insert(CONTENT_TYPE, content_type);
    }
    rquest::Response::from(response)
}

#[tokio::test]
async fn bounded_text_preserves_rquest_charset_and_bom_semantics() {
    let cases: &[(Option<&str>, &[u8], &str)] = &[
        (None, b"plain", "plain"),
        (None, b"a\xffb", "a\u{fffd}b"),
        (None, b"\xef\xbb\xbfhello", "hello"),
        (None, b"\xff\xfeA\x00", "A"),
        (None, b"\xfe\xff\x00A", "A"),
        (None, b"\xff\xfeA", "\u{fffd}"),
        (
            Some("text/plain; charset=windows-1252"),
            b"\x80\xe9",
            "\u{20ac}\u{e9}",
        ),
        (
            Some("text/plain; charset=\"ISO-8859-1\""),
            b"\x80",
            "\u{20ac}",
        ),
        (
            Some("text/plain; charset=invalid-label"),
            b"\xc3\xa9",
            "\u{e9}",
        ),
        (
            Some("not a mime; charset=windows-1252"),
            b"\x80",
            "\u{fffd}",
        ),
        (
            Some("text/plain; charset=windows-1252"),
            b"\xef\xbb\xbfA",
            "A",
        ),
        (None, b"", ""),
    ];
    for &(content_type, body, expected) in cases {
        let header = content_type.map(|value| value.parse::<HeaderValue>().unwrap());
        let original = response(body.to_vec(), header.clone())
            .text()
            .await
            .unwrap();
        assert_eq!(original, expected, "oracle for {content_type:?}");
        let bounded = collect_bounded_upstream_charset_text_with_provider(
            response(body.to_vec(), header),
            LABEL,
            PROVIDER,
        )
        .await
        .unwrap();
        assert_eq!(bounded, original, "bounded decoder for {content_type:?}");
    }
}

#[tokio::test]
async fn non_text_content_type_retains_default_decoding() {
    let header = HeaderValue::from_bytes(b"\x80").unwrap();
    let original = response(b"\xef\xbb\xbfA".to_vec(), Some(header.clone()))
        .text()
        .await
        .unwrap();
    let bounded = collect_bounded_upstream_charset_text_with_provider(
        response(b"\xef\xbb\xbfA".to_vec(), Some(header)),
        LABEL,
        PROVIDER,
    )
    .await
    .unwrap();
    assert_eq!(original, "A");
    assert_eq!(bounded, original);
}

struct StreamDrop(Arc<AtomicBool>);

impl Drop for StreamDrop {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

#[tokio::test]
async fn unknown_length_body_stops_at_limit_and_drops_its_stream() {
    const CHUNK_BYTES: usize = 1024 * 1024;
    let chunk = Bytes::from(vec![b'x'; CHUNK_BYTES]);
    let dropped = Arc::new(AtomicBool::new(false));
    let reads = Arc::new(AtomicUsize::new(0));
    let counter = reads.clone();
    let remaining = MAX_ACCUMULATED_UPSTREAM_BODY_BYTES / CHUNK_BYTES + 2;
    let stream = futures::stream::unfold(
        (remaining, StreamDrop(dropped.clone())),
        move |(remaining, guard)| {
            let chunk = chunk.clone();
            let counter = counter.clone();
            async move {
                if remaining == 0 {
                    return None;
                }
                counter.fetch_add(1, Ordering::SeqCst);
                Some((Ok::<_, std::io::Error>(chunk), (remaining - 1, guard)))
            }
        },
    );
    let response = response(rquest::Body::wrap_stream(stream), None);
    assert_eq!(response.content_length(), None);
    let error = collect_bounded_upstream_charset_text_with_provider(response, LABEL, PROVIDER)
        .await
        .expect_err("unknown-length stream must be bounded");
    assert_eq!(error.code.as_deref(), Some("upstream_body_too_large"));
    assert_eq!(error.provider_name.as_deref(), Some(PROVIDER));
    assert_eq!(
        reads.load(Ordering::SeqCst),
        MAX_ACCUMULATED_UPSTREAM_BODY_BYTES / CHUNK_BYTES + 1
    );
    assert!(dropped.load(Ordering::SeqCst));
}

fn failed_response() -> rquest::Response {
    let stream = futures::stream::once(async {
        Err::<Bytes, _>(std::io::Error::new(
            std::io::ErrorKind::ConnectionReset,
            "fixture body reset",
        ))
    });
    response(rquest::Body::wrap_stream(stream), None)
}

#[tokio::test]
async fn body_read_errors_keep_the_original_provider_classification() {
    let original = failed_response().text().await.unwrap_err();
    let expected = classify_network_error(&original, Some(PROVIDER));
    let actual =
        collect_bounded_upstream_charset_text_with_provider(failed_response(), LABEL, PROVIDER)
            .await
            .unwrap_err();
    assert_eq!(actual.message, expected.message);
    assert_eq!(actual.code, expected.code);
    assert_eq!(actual.provider_name, expected.provider_name);
    assert_eq!(actual.http_status, expected.http_status);
    assert_eq!(actual.retryable, expected.retryable);
}

#[tokio::test]
async fn cancelled_body_read_drops_the_pending_stream() {
    let dropped = Arc::new(AtomicBool::new(false));
    let guard = StreamDrop(dropped.clone());
    let stream = futures::stream::once(async move {
        let _guard = guard;
        futures::future::pending::<Result<Bytes, std::io::Error>>().await
    });
    let response = response(rquest::Body::wrap_stream(stream), None);
    let mut read = Box::pin(collect_bounded_upstream_charset_text_with_provider(
        response, LABEL, PROVIDER,
    ));
    assert!(futures::poll!(read.as_mut()).is_pending());
    assert!(!dropped.load(Ordering::SeqCst));
    drop(read);
    assert!(dropped.load(Ordering::SeqCst));
}

#[tokio::test]
async fn bounded_body_read_preserves_the_request_deadline() {
    use std::time::Duration;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    for bounded in [false, true] {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let serve = async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = [0; 4096];
            assert!(socket.read(&mut request).await.unwrap() > 0);
            socket
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 1\r\nConnection: close\r\n\r\n")
                .await
                .unwrap();
            tokio::time::sleep(Duration::from_secs(2)).await;
        };
        let read = async move {
            let response = rquest::Client::builder()
                .no_proxy()
                .build()
                .unwrap()
                .get(url)
                .timeout(Duration::from_millis(500))
                .send()
                .await
                .expect("response headers arrive before the body stalls");
            if bounded {
                collect_bounded_upstream_charset_text_with_provider(response, LABEL, PROVIDER)
                    .await
                    .unwrap_err()
            } else {
                classify_network_error(&response.text().await.unwrap_err(), Some(PROVIDER))
            }
        };
        tokio::time::timeout(Duration::from_secs(5), async {
            let ((), error) = tokio::join!(serve, read);
            assert!(matches!(error.kind, crate::error::ErrorKind::Timeout));
            assert_eq!(error.provider_name.as_deref(), Some(PROVIDER));
        })
        .await
        .expect("deadline fixture must terminate");
    }
}
