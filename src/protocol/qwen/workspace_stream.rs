//! Qwen 保持原诊断策略，Chat 字节准入由共享协议 owner 负责。
use crate::protocol::stream_error::ProtocolStreamError;
use bytes::Bytes;
use futures::Stream;
pub(crate) fn require_completion<S, E>(stream: S) -> impl Stream<Item = Result<Bytes, E>> + Send
where
    S: Stream<Item = Result<Bytes, E>> + Send + 'static,
    E: From<ProtocolStreamError> + Send + 'static,
{
    crate::protocol::openai::chat_stream::require_completion(stream, || {
        ProtocolStreamError::invalid_data(
            "invalid_qwen_workspace_stream: incomplete or invalid Chat stream",
        )
    })
}
