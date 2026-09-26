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

pub use accumulate::{accumulate_responses_sse_stream, accumulate_responses_stream};
pub use normalize::normalize_responses;
pub use pack::{pack_responses, pack_responses_bridge};
use response::normalize_arguments_value;
pub use response::{build_responses_success, unpack_responses_response};
pub use to_openai_chat::translate_responses_sse_to_openai_chat;
pub use to_responses::translate_openai_sse_to_responses;

#[cfg(test)]
use accumulate::{accumulate_responses_sse_bytes, accumulate_responses_sse_stream_with_limit};
#[cfg(test)]
use to_openai_chat::translate_responses_sse_to_openai_chat_with_limit;
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
