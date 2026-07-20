use bytes::Bytes;
use futures::Stream;
use serde_json::{json, Value};

use crate::error::GatewayError;
use crate::protocol::canonical::{CanonicalRelayRequest, CanonicalRelayResponse};

pub const QWEN_WEB_DEFAULT_CREATE_CHAT_PATH: &str = "/api/v2/chats/new";
pub const QWEN_WEB_DEFAULT_CHAT_COMPLETIONS_PATH: &str = "/api/v2/chat/completions";
pub const QWEN_WEB_DEFAULT_USER_AGENT: &str = "Mozilla/5.0";
pub const QWEN_WEB_DEFAULT_SEC_CH_UA: &str = "\"Chromium\";v=\"143\"";
pub const QWEN_WEB_DEFAULT_ACCEPT_LANGUAGE: &str = "zh-CN,zh;q=0.9,en-US;q=0.8,en;q=0.7";
pub const QWEN_WEB_DEFAULT_TIMEZONE: &str = "Asia/Shanghai";
pub const QWEN_WEB_DEFAULT_VERSION: &str = "0.1.13";
pub const QWEN_WEB_DEFAULT_BX_VERSION: &str = "2.5.31";
pub const QWEN_WEB_BROWSER_CHALLENGE_REQUIRED_CODE: &str = "qwen_web_browser_challenge_required";
pub const QWEN_WEB_SESSION_INVALID_CODE: &str = "qwen_web_session_invalid";

fn compiled_out_error() -> GatewayError {
    GatewayError::conflict("Qwen Web reverse implementation line was compiled out.")
        .with_code("gateway_provider_line_compiled_out")
}

pub fn pack_create_chat(_model: &str) -> Value {
    json!({})
}

pub fn pack_qwen_web(
    _req: &CanonicalRelayRequest,
    _model: &str,
    _chat_id: &str,
) -> Result<Value, GatewayError> {
    Err(compiled_out_error())
}

pub fn classify_qwen_web_http_error(
    _status: u16,
    _content_type: Option<&str>,
    _body: &str,
) -> GatewayError {
    compiled_out_error()
}

pub fn response_indicates_browser_challenge(
    _status: u16,
    _content_type: Option<&str>,
    _body: &str,
) -> bool {
    false
}

pub fn response_indicates_session_invalid(
    _status: u16,
    _content_type: Option<&str>,
    _body: &str,
) -> bool {
    false
}

pub async fn accumulate_qwen_web_stream(
    _response: rquest::Response,
    _model: &str,
) -> Result<CanonicalRelayResponse, GatewayError> {
    Err(compiled_out_error())
}

pub fn translate_qwen_web_frame_to_openai_sse(
    _data_str: &str,
    _model: &str,
    _response_id: &str,
    _created: i64,
) -> Option<Vec<u8>> {
    None
}

pub fn translate_qwen_web_stream(
    _inner: impl Stream<Item = Result<Bytes, rquest::Error>> + Send + 'static,
    _model: String,
) -> impl Stream<Item = Result<Bytes, rquest::Error>> + Send + 'static {
    futures::stream::once(async { Ok(Bytes::from_static(b"data: [DONE]\n\n")) })
}
