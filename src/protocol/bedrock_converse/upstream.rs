//! Inbound AWS frames and completion state are independent of Accio's SSE protocol.
mod capture;
mod frame;
mod headers;
mod reasoning;
mod relay;
mod response;
mod state;
mod stream;
mod turn;

pub(crate) use relay::Relay;
pub(crate) use response::decode_eventstream;
pub use response::{accumulate, unpack};
pub use stream::translate;
pub(crate) use stream::{translate_public, translate_scoped};

use crate::protocol::stream_error::ProtocolStreamError;

pub(super) fn invalid(message: &'static str) -> ProtocolStreamError {
    // Never include a raw header, JSON value, tool argument, or upstream error message.
    ProtocolStreamError::invalid_data(format!("bedrock_stream_invalid: {message}"))
}

fn upstream_failure(kind: &str) -> ProtocolStreamError {
    super::exception::failure(kind)
}
