//! Streaming decompression with one end-to-end deadline and explicit body limits.
use crate::pool::Lease;
use async_compression::tokio::bufread::{BrotliDecoder, GzipDecoder, ZlibDecoder, ZstdDecoder};
use futures::{StreamExt, TryStreamExt};
use http::{Response, Uri};
use http_body_util::BodyExt;
use rquest::ResponseBuilderExt;
use std::{io, pin::Pin};
use tokio::{
    io::{AsyncRead, BufReader},
    time::Instant,
};
use tokio_util::io::{ReaderStream, StreamReader};
use wreq_proto::body::Incoming;

const LIMIT: usize = 64 * 1024 * 1024;
type Reader = Box<dyn AsyncRead + Unpin + Send>;
type ChunkStream = Pin<Box<dyn futures::Stream<Item = io::Result<bytes::Bytes>> + Send>>;

struct State {
    stream: Option<ChunkStream>,
    lease: Option<Lease>,
    deadline: Instant,
    total: usize,
}

pub(crate) fn adapt(
    response: Response<Incoming>,
    uri: Uri,
    method: http::Method,
    lease: Lease,
    deadline: Instant,
) -> rquest::Response {
    let (mut parts, incoming) = response.into_parts();
    let encodings: Vec<_> = parts
        .headers
        .get_all(http::header::CONTENT_ENCODING)
        .iter()
        // Invalid or unknown components keep the entire representation opaque.
        .flat_map(|v| v.to_str().unwrap_or("unsupported").split(','))
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_ascii_lowercase)
        .collect();
    let mut raw_total = 0usize;
    let raw = incoming
        .into_data_stream()
        .map_err(|_| io::Error::other("CPA response read failed"))
        .and_then(move |chunk| {
            raw_total = raw_total.saturating_add(chunk.len());
            futures::future::ready(if raw_total > LIMIT {
                Err(io::Error::other(crate::BodyLimitExceeded))
            } else {
                Ok(chunk)
            })
        });
    let mut reader: Reader = Box::new(StreamReader::new(raw));
    let no_body = method == http::Method::HEAD
        || parts.status.is_informational()
        || matches!(parts.status.as_u16(), 204 | 304)
        || parts
            .headers
            .get(http::header::CONTENT_LENGTH)
            .is_some_and(|v| v == "0");
    // Retain Gateway's decoding contract, rather than Go's auto-added-gzip-only API contract.
    // Bound nesting so untrusted response metadata cannot create an unbounded decoder stack.
    if !no_body
        && !encodings.is_empty()
        && encodings.len() <= 8
        && encodings
            .iter()
            .all(|e| matches!(e.as_str(), "gzip" | "deflate" | "br" | "zstd"))
    {
        for encoding in encodings.iter().rev() {
            let input = BufReader::new(reader);
            reader = match encoding.as_str() {
                "gzip" => {
                    let mut decoder = GzipDecoder::new(input);
                    decoder.multiple_members(true);
                    Box::new(decoder)
                }
                "deflate" => Box::new(ZlibDecoder::new(input)),
                "br" => Box::new(BrotliDecoder::new(input)),
                _ => Box::new(ZstdDecoder::new(input)),
            };
        }
        parts.headers.remove(http::header::CONTENT_ENCODING);
        parts.headers.remove(http::header::CONTENT_LENGTH);
    }
    let state = State {
        stream: Some(Box::pin(ReaderStream::with_capacity(reader, 32768))),
        lease: Some(lease),
        deadline,
        total: 0,
    };
    let stream = futures::stream::unfold(state, |mut state| async move {
        let source = state.stream.as_mut()?;
        let value = match tokio::time::timeout_at(state.deadline, source.next()).await {
            Err(_) => Some(Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "CPA response deadline",
            ))),
            Ok(value) => value,
        };
        let value = value.map(|chunk| {
            chunk.and_then(|chunk| {
                state.total = state.total.saturating_add(chunk.len());
                if state.total > LIMIT {
                    Err(io::Error::other(crate::BodyLimitExceeded))
                } else {
                    Ok(chunk)
                }
            })
        });
        if value.as_ref().is_none_or(|v| v.is_err()) {
            // Release/close before publishing EOF or error; abandoned streams drop State too.
            state.stream.take();
            if value.is_none() {
                if let Some(lease) = state.lease.as_mut() {
                    lease.body_complete = true;
                }
            }
            state.lease.take();
        }
        value.map(|value| (value, state))
    });
    let mut response = Response::builder()
        .status(parts.status)
        .version(parts.version)
        .uri(uri)
        .body(rquest::Body::wrap_stream(stream))
        .expect("validated HTTP response parts");
    *response.headers_mut() = parts.headers;
    response.extensions_mut().extend(parts.extensions);
    response.into()
}
