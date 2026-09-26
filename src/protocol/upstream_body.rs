//! Bounded whole-body reads for adapters that must collapse an upstream stream.
//! Streaming passthrough bypasses this module and retains its existing backpressure.

use bytes::Bytes;
use futures::{Stream, StreamExt};

use crate::error::{classify_network_error, GatewayError};

pub(crate) const MAX_ACCUMULATED_UPSTREAM_BODY_BYTES: usize = 64 * 1024 * 1024;

pub(crate) async fn collect_bounded_upstream_body(
    response: rquest::Response,
    body_label: &'static str,
) -> Result<Vec<u8>, GatewayError> {
    collect_bounded_upstream_response(response, body_label, None).await
}

pub(crate) async fn collect_bounded_upstream_body_with_provider(
    response: rquest::Response,
    body_label: &'static str,
    provider: &str,
) -> Result<Vec<u8>, GatewayError> {
    collect_bounded_upstream_response(response, body_label, Some(provider)).await
}

pub(crate) async fn collect_bounded_upstream_text(
    response: rquest::Response,
    body_label: &'static str,
) -> Result<String, GatewayError> {
    let body = collect_bounded_upstream_body(response, body_label).await?;
    String::from_utf8(body).map_err(|error| {
        GatewayError::server_error(format!(
            "{body_label} was not valid UTF-8: {}",
            error.utf8_error()
        ))
        .with_code("upstream_body_invalid_utf8")
    })
}

pub(crate) async fn collect_bounded_upstream_text_with_provider(
    response: rquest::Response,
    body_label: &'static str,
    provider: &str,
) -> Result<String, GatewayError> {
    let body = collect_bounded_upstream_response(response, body_label, Some(provider)).await?;
    Ok(decode_lossy_utf8(body))
}

pub(crate) async fn collect_bounded_upstream_charset_text_with_provider(
    response: rquest::Response,
    body_label: &'static str,
    provider: &str,
) -> Result<String, GatewayError> {
    let content_type = response
        .headers()
        .get(rquest::header::CONTENT_TYPE)
        .cloned();
    let body = collect_bounded_upstream_response(response, body_label, Some(provider)).await?;
    // Reuse rquest's charset/BOM decoder on bounded bytes, without another network read
    // or transport headers that could describe an already-decoded response body.
    let mut bounded = axum::http::Response::new(body);
    if let Some(content_type) = content_type {
        bounded
            .headers_mut()
            .insert(rquest::header::CONTENT_TYPE, content_type);
    }
    rquest::Response::from(bounded)
        .text()
        .await
        .map_err(|error| classify_network_error(&error, Some(provider)))
}

async fn collect_bounded_upstream_response(
    response: rquest::Response,
    body_label: &'static str,
    provider: Option<&str>,
) -> Result<Vec<u8>, GatewayError> {
    if content_length_exceeds_limit(
        response.content_length(),
        MAX_ACCUMULATED_UPSTREAM_BODY_BYTES,
    ) {
        return Err(attach_optional_provider(
            upstream_body_too_large_error(body_label, MAX_ACCUMULATED_UPSTREAM_BODY_BYTES),
            provider,
        ));
    }

    collect_bounded_upstream_stream_with_limit_and_provider(
        response.bytes_stream(),
        body_label,
        MAX_ACCUMULATED_UPSTREAM_BODY_BYTES,
        provider,
    )
    .await
}

async fn collect_bounded_upstream_stream_with_limit_and_provider<S>(
    stream: S,
    body_label: &'static str,
    max_bytes: usize,
    provider: Option<&str>,
) -> Result<Vec<u8>, GatewayError>
where
    S: Stream<Item = Result<Bytes, rquest::Error>>,
{
    futures::pin_mut!(stream);
    let mut body = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|error| match provider {
            Some(provider) => classify_network_error(&error, Some(provider)),
            None => GatewayError::server_error(format!("failed to read {body_label}: {error}"))
                .with_code("upstream_body_read_failed"),
        })?;
        let next_len = body
            .len()
            .checked_add(chunk.len())
            .filter(|length| *length <= max_bytes)
            .ok_or_else(|| {
                attach_optional_provider(
                    upstream_body_too_large_error(body_label, max_bytes),
                    provider,
                )
            })?;
        body.try_reserve(next_len - body.len()).map_err(|error| {
            attach_optional_provider(
                GatewayError::server_error(format!(
                    "failed to reserve {body_label} accumulation buffer: {error}"
                ))
                .with_code("upstream_body_buffer_allocation_failed"),
                provider,
            )
        })?;
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

fn content_length_exceeds_limit(content_length: Option<u64>, max_bytes: usize) -> bool {
    content_length.is_some_and(|length| length > max_bytes as u64)
}

fn upstream_body_too_large_error(body_label: &str, max_bytes: usize) -> GatewayError {
    GatewayError::server_error(format!(
        "{body_label} exceeded the {max_bytes}-byte accumulation limit."
    ))
    .with_code("upstream_body_too_large")
}

fn attach_optional_provider(error: GatewayError, provider: Option<&str>) -> GatewayError {
    match provider {
        Some(provider) => with_provider(error, provider),
        None => error,
    }
}

fn with_provider(error: GatewayError, provider: &str) -> GatewayError {
    error.with_provider(provider)
}

fn decode_lossy_utf8(body: Vec<u8>) -> String {
    match String::from_utf8(body) {
        Ok(text) => text,
        Err(error) => String::from_utf8_lossy(error.as_bytes()).into_owned(),
    }
}

#[cfg(test)]
#[path = "upstream_body_charset_tests.rs"]
mod charset_tests;

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};

    use super::*;

    async fn response_with_declared_length(
        declared_length: u64,
    ) -> (rquest::Response, std::thread::JoinHandle<()>) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind response server");
        let addr = listener.local_addr().expect("read response server addr");
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept request");
            let mut request = [0_u8; 4096];
            let _ = stream.read(&mut request);
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {declared_length}\r\n\r\n"
            );
            stream
                .write_all(response.as_bytes())
                .expect("write response headers");
        });
        let response = rquest::Client::new()
            .get(format!("http://{addr}"))
            .send()
            .await
            .expect("fetch response");
        (response, server)
    }

    #[test]
    fn content_length_limit_accepts_exact_size_and_rejects_larger_bodies() {
        assert!(!content_length_exceeds_limit(Some(8), 8));
        assert!(content_length_exceeds_limit(Some(9), 8));
        assert!(!content_length_exceeds_limit(None, 8));
    }

    #[tokio::test]
    async fn bounded_stream_accepts_payload_at_limit_without_reordering_chunks() {
        let stream = futures::stream::iter(vec![
            Ok::<_, rquest::Error>(Bytes::from_static(b"abcd")),
            Ok::<_, rquest::Error>(Bytes::from_static(b"efgh")),
        ]);

        let body =
            collect_bounded_upstream_stream_with_limit_and_provider(stream, "test body", 8, None)
                .await
                .expect("body at the limit should be accepted");

        assert_eq!(body, b"abcdefgh");
    }

    #[tokio::test]
    async fn bounded_stream_rejects_chunked_payload_above_limit() {
        let stream = futures::stream::iter(vec![
            Ok::<_, rquest::Error>(Bytes::from_static(b"abcd")),
            Ok::<_, rquest::Error>(Bytes::from_static(b"efghi")),
        ]);

        let error =
            collect_bounded_upstream_stream_with_limit_and_provider(stream, "test body", 8, None)
                .await
                .expect_err("oversized chunked body should be rejected");

        assert_eq!(error.code.as_deref(), Some("upstream_body_too_large"));
        assert!(error.message.contains("8-byte accumulation limit"));
    }

    #[tokio::test]
    async fn provider_stream_limit_preserves_provider_contract() {
        let stream = futures::stream::iter(vec![Ok::<_, rquest::Error>(Bytes::from_static(
            b"123456789",
        ))]);

        let error = collect_bounded_upstream_stream_with_limit_and_provider(
            stream,
            "Producer.ai SSE body",
            8,
            Some("producer_compatible"),
        )
        .await
        .expect_err("oversized provider body should be rejected");

        assert_eq!(error.code.as_deref(), Some("upstream_body_too_large"));
        assert_eq!(error.provider_name.as_deref(), Some("producer_compatible"));
    }

    #[test]
    fn lossy_decoder_preserves_valid_text_and_replaces_invalid_bytes() {
        assert_eq!(
            decode_lossy_utf8("Neuro 网关".as_bytes().to_vec()),
            "Neuro 网关"
        );
        assert_eq!(decode_lossy_utf8(vec![b'a', 0xff, b'b']), "a\u{fffd}b");
    }

    #[tokio::test]
    async fn provider_text_reader_rejects_oversized_declared_length_before_body_read() {
        let declared_length = MAX_ACCUMULATED_UPSTREAM_BODY_BYTES as u64 + 1;
        let (response, server) = response_with_declared_length(declared_length).await;

        let error = collect_bounded_upstream_text_with_provider(
            response,
            "Producer.ai SSE body",
            "producer_compatible",
        )
        .await
        .expect_err("oversized declared body should fail before allocation");
        server.join().expect("response server exits");

        assert_eq!(error.code.as_deref(), Some("upstream_body_too_large"));
        assert_eq!(error.provider_name.as_deref(), Some("producer_compatible"));
        assert!(error.message.contains("67108864-byte accumulation limit"));
    }
}
