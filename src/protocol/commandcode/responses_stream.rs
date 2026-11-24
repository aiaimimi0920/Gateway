//! Command Code native Responses policy, with shared bounded framing and clean EOF checks.
use crate::protocol::stream_error::ProtocolStreamError;
use bytes::Bytes;
use futures::Stream;

pub(crate) fn guard<S, E>(stream: S, model: String) -> impl Stream<Item = Result<Bytes, E>> + Send
where
    S: Stream<Item = Result<Bytes, E>> + Send + 'static,
    E: From<ProtocolStreamError> + Send + 'static,
{
    crate::protocol::responses::native_stream::guard(
        stream,
        model,
        true,
        |value, _, terminal| {
            !terminal || super::responses::validate_native(&value["response"]).is_ok()
        },
        super::stream_error,
    )
}

#[cfg(test)]
#[path = "responses_stream_tests.rs"]
mod tests;
