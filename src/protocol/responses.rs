// ---------------------------------------------------------------------------
// OpenAI Responses API adapter — pack / unpack / builder helpers
//
// Handles the simpler `POST /v1/responses` format.
// ---------------------------------------------------------------------------

mod accumulate;
mod normalize;
mod pack;
mod response;
mod stream_support;
mod to_openai_chat;
mod to_responses;
mod to_responses_events;
use crate::protocol::stream_error::StreamError;
use crate::protocol::stream_error_legacy::with_transport_error;

pub use accumulate::{
    accumulate_responses_sse_stream, accumulate_responses_sse_stream_with_error,
    accumulate_responses_stream,
};
pub use normalize::normalize_responses;
pub use pack::{pack_responses, pack_responses_bridge};
use response::normalize_arguments_value;
pub use response::{build_responses_success, unpack_responses_response};
pub use to_openai_chat::translate_responses_sse_to_openai_chat_with_error;
pub use to_responses::{
    translate_openai_sse_to_responses, translate_openai_sse_to_responses_with_error,
};

#[cfg(test)]
use accumulate::{accumulate_responses_sse_bytes, accumulate_responses_sse_stream_with_limit};
/// 旧客户端入口留在 facade；协议实现只依赖调用者的错误类型。
pub fn translate_responses_sse_to_openai_chat(
    inner: impl futures::Stream<Item = Result<bytes::Bytes, rquest::Error>> + Send + 'static,
    model: String,
) -> impl futures::Stream<Item = Result<bytes::Bytes, StreamError<rquest::Error>>> + Send + 'static
{
    translate_responses_sse_to_openai_chat_with_error(with_transport_error(inner), model)
}

#[cfg(test)]
fn translate_responses_sse_to_openai_chat_with_limit(
    inner: impl futures::Stream<Item = Result<bytes::Bytes, rquest::Error>> + Send + 'static,
    model: String,
    max_frame_bytes: usize,
) -> impl futures::Stream<Item = Result<bytes::Bytes, StreamError<rquest::Error>>> + Send + 'static
{
    to_openai_chat::translate_responses_sse_to_openai_chat_with_limit_and_error(
        with_transport_error(inner),
        model,
        max_frame_bytes,
    )
}
#[cfg(test)]
use to_responses::translate_openai_sse_to_responses_with_limit;

// ---------------------------------------------------------------------------
// Streaming translators
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests;
