//! Serial stream delivery: backpressure and Cursor terminal/publication ownership.
use super::{cursor_websocket, websocket};
use crate::protocol::{
    stream_decode::{BoundedSseDecoder, DecodeStep},
    stream_error::StreamError,
};
use axum::extract::ws::{Message, WebSocket};
use bytes::Bytes;
use futures::{stream::SplitSink, stream::SplitStream, SinkExt, Stream, StreamExt};
use serde_json::Value;
use std::time::Duration;

pub(super) async fn relay(
    writer: &mut SplitSink<WebSocket, Message>,
    reader: &mut SplitStream<WebSocket>,
    mut stream: impl Stream<Item = Result<Bytes, StreamError<rquest::Error>>> + Unpin,
    mut cursor_turn: Option<cursor_websocket::Turn>,
    cursor_connection: &mut cursor_websocket::Connection,
) -> Result<(), ()> {
    let mut decoder = if cursor_turn.is_some() {
        BoundedSseDecoder::new_strict(16 * 1024 * 1024 + 1024)
    } else {
        BoundedSseDecoder::new(16 * 1024 * 1024 + 1024)
    };
    let mut upstream_failed = false;
    loop {
        let chunk = tokio::select! {
            biased;
            chunk = stream.next() => chunk,
            // Once the terminal is buffered, drain the finite Cursor phase
            // without consuming any frames belonging to the next turn.
            // The terminal write and pause publication follow strict EOF.
            message = reader.next(), if !cursor_turn.as_ref().is_some_and(|turn| turn.terminal_received()) => {
                if !websocket::control(writer, message).await { return Err(()); }
                continue;
            },
        };
        match chunk {
            Some(Ok(chunk)) => decoder.push_chunk(chunk),
            Some(Err(_)) => {
                if !upstream_failed {
                    websocket::send_error(writer, "Responses WebSocket stream interrupted").await?;
                }
                upstream_failed = true;
                break;
            }
            None => break,
        }
        loop {
            match decoder.decode_next() {
                DecodeStep::Frame(frame) => {
                    if let Ok(event) = serde_json::from_str::<Value>(&frame.data) {
                        upstream_failed |= matches!(
                            event["type"].as_str(),
                            Some("error" | "response.failed" | "response.incomplete")
                        );
                    }
                    let forward = match cursor_turn.as_mut() {
                        Some(turn) => turn.accept(&frame.data)?,
                        None => true,
                    };
                    if forward {
                        // Await every write: slow clients backpressure upstream.
                        tokio::time::timeout(
                            Duration::from_secs(30),
                            writer.send(Message::Text(frame.data)),
                        )
                        .await
                        .map_err(|_| ())?
                        .map_err(|_| ())?;
                    }
                }
                DecodeStep::NeedInput => break,
                DecodeStep::Error(_) => return Err(()),
            }
        }
    }
    if let Some(turn) = cursor_turn {
        if upstream_failed || decoder.finish_strict().is_err() {
            return Err(());
        }
        let mut ready = turn.prepare()?;
        tokio::time::timeout(
            Duration::from_secs(30),
            writer.send(Message::Text(std::mem::take(&mut ready.data))),
        )
        .await
        .map_err(|_| ())?
        .map_err(|_| ())?;
        cursor_connection.finish(ready)?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "websocket_delivery_tests.rs"]
mod tests;
