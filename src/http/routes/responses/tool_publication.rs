//! Keep a parked Cursor Run cancellable through async finalization and HTTP body delivery.
use crate::protocol::cursor_remote::runtime::Publication;
use axum::{
    body::{Body, HttpBody},
    response::Response,
};
use futures::StreamExt;
use std::sync::atomic::Ordering;

pub(super) fn attach(response: Response, mut guard: Publication) -> Response {
    let (parts, body) = response.into_parts();
    let sse = parts
        .headers
        .get("content-type")
        .is_some_and(|v| v == "text/event-stream");
    let expected = body.size_hint().exact();
    let (stream, terminal) =
        crate::upstream::stream::observe_responses_terminal(Box::pin(body.into_data_stream()));
    let mut delivered = 0u64;
    let stream = stream.map(move |result| {
        if let Ok(bytes) = &result {
            delivered += bytes.len() as u64;
            if (sse && terminal.load(Ordering::Acquire)) || (!sse && expected == Some(delivered)) {
                // Handed to HTTP, not a TCP acknowledgement or a remote execution receipt.
                guard.commit();
            }
        }
        result
    });
    Response::from_parts(parts, Body::from_stream(stream))
}
