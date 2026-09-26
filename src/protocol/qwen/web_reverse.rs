mod accumulation;
mod diagnostics;
mod http_errors;
mod request;
mod response_value;
mod stream;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod usage_arithmetic_contract;

pub use accumulation::accumulate_qwen_web_stream;
pub use http_errors::{
    classify_qwen_web_http_error, response_indicates_browser_challenge,
    response_indicates_session_invalid,
};
pub use request::{pack_create_chat, pack_qwen_web};
pub use stream::{translate_qwen_web_frame_to_openai_sse, translate_qwen_web_stream};

pub const QWEN_WEB_DEFAULT_CREATE_CHAT_PATH: &str = "/api/v2/chats/new";
pub const QWEN_WEB_DEFAULT_CHAT_COMPLETIONS_PATH: &str = "/api/v2/chat/completions";
pub const QWEN_WEB_DEFAULT_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/143.0.0.0 Safari/537.36 Edg/143.0.0.0";
pub const QWEN_WEB_DEFAULT_SEC_CH_UA: &str =
    "\"Microsoft Edge\";v=\"143\", \"Chromium\";v=\"143\", \"Not A(Brand\";v=\"24\"";
pub const QWEN_WEB_DEFAULT_ACCEPT_LANGUAGE: &str = "zh-CN,zh;q=0.9,en-US;q=0.8,en;q=0.7";
pub const QWEN_WEB_DEFAULT_TIMEZONE: &str = "Mon Dec 08 2025 17:28:55 GMT+0800";
pub const QWEN_WEB_DEFAULT_VERSION: &str = "0.1.13";
pub const QWEN_WEB_DEFAULT_BX_VERSION: &str = "2.5.31";
pub const QWEN_WEB_BROWSER_CHALLENGE_REQUIRED_CODE: &str = "qwen_web_browser_challenge_required";
pub const QWEN_WEB_SESSION_INVALID_CODE: &str = "qwen_web_session_invalid";
