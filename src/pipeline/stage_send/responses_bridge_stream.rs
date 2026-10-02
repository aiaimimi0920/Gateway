//! The buffered Anthropic bridge owns the transport-to-protocol error boundary.
//! Failure still becomes the same GatewayError at accumulation, without treating
//! a local decoder failure as a transport/connect error along the way.
use bytes::Bytes;
use futures::{Stream, TryStreamExt};
use serde_json::Value;

use super::tool_stream::{wrap_injected_openai_stream, ByteStream};
use crate::error::GatewayError;
use crate::protocol::canonical::{CanonicalRelayResponse, CanonicalTool};
use crate::protocol::stream_error::StreamError;
use crate::protocol::{accio, responses};

pub(super) async fn accumulate<T: std::fmt::Display + Send + 'static>(
    inner: impl Stream<Item = Result<Bytes, T>> + Send + 'static,
    req_id: &uuid::Uuid,
    model: &str,
    tools: Vec<CanonicalTool>,
    tool_choice: Option<Value>,
    conversation_hint: Option<String>,
) -> Result<CanonicalRelayResponse, GatewayError> {
    let transport: ByteStream<StreamError<T>> = Box::pin(inner.map_err(StreamError::Transport));
    let openai = Box::pin(accio::translate_anthropic_like_stream_to_openai_with_error(
        transport,
        model.to_string(),
    ));
    let openai = wrap_injected_openai_stream(
        openai,
        true,
        req_id,
        model,
        tools,
        tool_choice,
        conversation_hint,
    );
    let responses = Box::pin(responses::translate_openai_sse_to_responses_with_error(
        openai,
        model.to_string(),
    ));
    // Returning an error drops the entire owned chain before route feedback runs.
    responses::accumulate_responses_sse_stream_with_error(responses, model).await
}

#[cfg(test)]
#[path = "responses_bridge_stream_tests.rs"]
mod tests;
