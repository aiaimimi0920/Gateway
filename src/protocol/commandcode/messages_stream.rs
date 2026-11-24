//! Command Code error classification for the shared native Messages guard.
use crate::protocol::{anthropic::native_stream, stream_error::ProtocolStreamError};
use bytes::Bytes;
use futures::Stream;

pub(crate) fn guard<S, E>(input: S, model: String) -> impl Stream<Item = Result<Bytes, E>> + Send
where
    S: Stream<Item = Result<Bytes, E>> + Send + 'static,
    E: From<ProtocolStreamError> + Send + 'static,
{
    native_stream::guard(input, model, super::stream_error, true)
}
