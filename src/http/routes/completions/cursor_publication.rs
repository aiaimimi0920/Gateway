//! Cursor tool phases are already buffered at the native handoff. Keep this
//! finite HTTP body unpublished until validated EOF, then hand it to the consumer.
//! A discarded body owns and cancels the pause; this is not a TCP acknowledgement.
use crate::protocol::{
    cursor_remote::runtime::Publication,
    stream_decode::{BoundedSseDecoder, DecodeStep},
};
use axum::{body::Body, response::Response};
use bytes::{Bytes, BytesMut};
use futures::StreamExt;
use serde_json::Value;
use std::{io, time::Duration};

const LIMIT: usize = 1024 * 1024;

pub(super) fn attach(response: Response, guard: Publication) -> Response {
    attach_validated(response, guard, |bytes, sse| {
        if sse {
            validate_sse(bytes)
        } else {
            validate_json(&bytes)
        }
    })
}

pub(in crate::http::routes) fn attach_validated(
    response: Response,
    mut guard: Publication,
    validate: impl FnOnce(Bytes, bool) -> Result<(), io::Error> + Send + 'static,
) -> Response {
    let (parts, body) = response.into_parts();
    let sse = parts
        .headers
        .get("content-type")
        .is_some_and(|v| v == "text/event-stream");
    let stream = futures::stream::once(async move {
        let bytes = tokio::time::timeout(Duration::from_secs(30), collect(body))
            .await
            .map_err(|_| invalid())??;
        validate(bytes.clone(), sse)?;
        if !guard.commit() {
            return Err(invalid());
        }
        Ok::<_, io::Error>(bytes)
    });
    Response::from_parts(parts, Body::from_stream(stream))
}

fn invalid() -> io::Error {
    io::Error::other("Cursor Chat tool phase was not completely delivered")
}

async fn collect(body: Body) -> Result<Bytes, io::Error> {
    let mut stream = body.into_data_stream();
    let mut result = BytesMut::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| invalid())?;
        if chunk.len() > LIMIT - result.len() {
            return Err(invalid());
        }
        result.extend_from_slice(&chunk);
    }
    Ok(result.freeze())
}

fn choices(value: &Value) -> Result<&Value, io::Error> {
    let choices = value["choices"]
        .as_array()
        .filter(|c| c.len() == 1)
        .ok_or_else(invalid)?;
    if choices[0]["index"] != 0 || value.get("error").is_some() {
        return Err(invalid());
    }
    Ok(&choices[0])
}

fn tool_calls(value: &Value) -> Result<(), io::Error> {
    let calls = value
        .as_array()
        .filter(|v| v.len() == 1)
        .ok_or_else(invalid)?;
    let call = &calls[0];
    let kind = call["type"]
        .as_str()
        .filter(|s| ["function", "custom"].contains(s))
        .ok_or_else(invalid)?;
    let input = if kind == "custom" {
        "input"
    } else {
        "arguments"
    };
    if call["id"].as_str().is_none_or(str::is_empty)
        || call[kind]["name"].as_str().is_none_or(str::is_empty)
        || !call[kind][input].is_string()
    {
        return Err(invalid());
    }
    Ok(())
}

fn validate_json(bytes: &[u8]) -> Result<(), io::Error> {
    let value: Value = serde_json::from_slice(bytes).map_err(|_| invalid())?;
    let choice = choices(&value)?;
    if value["object"] != "chat.completion"
        || choice["finish_reason"] != "tool_calls"
        || choice["message"]["role"] != "assistant"
    {
        return Err(invalid());
    }
    tool_calls(&choice["message"]["tool_calls"])
}

fn validate_sse(bytes: Bytes) -> Result<(), io::Error> {
    let mut decoder = BoundedSseDecoder::new_strict(LIMIT);
    decoder.push_chunk(bytes);
    let (mut called, mut finished, mut done) = (false, false, false);
    loop {
        let frame = match decoder.decode_next() {
            DecodeStep::Frame(frame) => frame,
            DecodeStep::NeedInput => break,
            DecodeStep::Error(_) => return Err(invalid()),
        };
        if done || frame.event_name.is_some() {
            return Err(invalid());
        }
        if frame.data == "[DONE]" && finished {
            done = true;
            continue;
        }
        if finished {
            return Err(invalid());
        }
        let value: Value = serde_json::from_str(&frame.data).map_err(|_| invalid())?;
        let choice = choices(&value)?;
        if value["object"] != "chat.completion.chunk" {
            return Err(invalid());
        }
        if let Some(calls) = choice["delta"].get("tool_calls") {
            if called {
                return Err(invalid());
            }
            tool_calls(calls)?;
            called = true;
        }
        if !choice["finish_reason"].is_null() {
            if !called || choice["finish_reason"] != "tool_calls" {
                return Err(invalid());
            }
            finished = true;
        }
    }
    decoder.finish_strict().map_err(|_| invalid())?;
    if !done {
        return Err(invalid());
    }
    Ok(())
}

#[cfg(test)]
#[path = "cursor_publication_tests.rs"]
mod tests;
