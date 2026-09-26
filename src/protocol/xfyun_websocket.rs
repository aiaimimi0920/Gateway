mod connection;
mod frames;
mod payload_fields;
mod request;
mod signing;
mod stream;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod usage_arithmetic_contract;

pub use connection::execute_nonstream;
pub use request::{build_request_plan, pack_request, supports_endpoint};
pub use signing::build_signed_websocket_url;
pub use stream::execute_stream_as_openai_sse;

use crate::protocol::canonical::TokenUsage;
use bytes::Bytes;
use futures::Stream;
use std::pin::Pin;
use tokio::net::TcpStream;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

pub const XFYUN_WEBSOCKET_DEFAULT_PATH: &str = "/v1.1/chat";

type XfyunWsSocket = WebSocketStream<MaybeTlsStream<TcpStream>>;
type OpenAiSseByteStream =
    Pin<Box<dyn Stream<Item = Result<Bytes, rquest::Error>> + Send + 'static>>;

#[derive(Debug, Default)]
struct ParsedXfyunFrame {
    content_fragments: Vec<String>,
    usage: Option<TokenUsage>,
    done: bool,
}
