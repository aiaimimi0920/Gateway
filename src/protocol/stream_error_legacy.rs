//! 旧入口接收客户端流，但只向领域错误提升；绝不反向伪造 HTTP 错误。
use super::stream_error::StreamError;
use bytes::Bytes;
use futures::{Stream, TryStreamExt};

pub(super) fn with_transport_error(
    inner: impl Stream<Item = Result<Bytes, rquest::Error>> + Send + 'static,
) -> impl Stream<Item = Result<Bytes, StreamError<rquest::Error>>> + Send + 'static {
    inner.map_err(StreamError::Transport)
}
