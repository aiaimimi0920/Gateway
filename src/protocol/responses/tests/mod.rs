use super::*;
use bytes::Bytes;
use futures::{Stream, StreamExt};
use serde_json::{json, Value};

use crate::protocol::accio;
use crate::protocol::canonical::{
    CanonicalMessage, CanonicalRelayRequest, CanonicalTool, CanonicalToolCall, ContentPart,
    EndpointKind, MessageRole, ProtocolFamily, TokenUsage,
};
use crate::protocol::sse_parse::{
    format_sse_event, parse_sse_line as parse_sse_frame_line, SseFrame, SseParseState,
};

fn make_bytes_stream(
    chunks: Vec<Result<Bytes, rquest::Error>>,
) -> impl Stream<Item = Result<Bytes, rquest::Error>> + Send + 'static {
    futures::stream::iter(chunks)
}

async fn collect_sse_frames<E: std::fmt::Debug + Send + 'static>(
    stream: impl Stream<Item = Result<Bytes, E>> + Send + 'static,
) -> Vec<SseFrame> {
    let chunks = stream.collect::<Vec<_>>().await;
    let mut state = SseParseState::new();
    let mut frames = Vec::new();

    for chunk in chunks {
        let bytes = chunk.unwrap();
        let text = String::from_utf8_lossy(&bytes);
        for line in text.split('\n') {
            if let Some(frame) = parse_sse_frame_line(line, &mut state) {
                frames.push(frame);
            }
        }
    }

    if let Some(frame) = parse_sse_frame_line("", &mut state) {
        frames.push(frame);
    }

    frames
}

async fn collect_stream_output<E: std::fmt::Display + Send + 'static>(
    stream: impl Stream<Item = Result<Bytes, E>> + Send + 'static,
) -> (String, Vec<String>) {
    let mut output = String::new();
    let mut errors = Vec::new();
    for item in stream.collect::<Vec<_>>().await {
        match item {
            Ok(bytes) => output.push_str(&String::from_utf8_lossy(&bytes)),
            Err(error) => errors.push(error.to_string()),
        }
    }
    (output, errors)
}

mod accumulate_tests;
mod normalize_response_tests;
mod pack_tests;
mod to_openai_chat_tests;
mod to_responses_order_tests;
mod to_responses_tests;
