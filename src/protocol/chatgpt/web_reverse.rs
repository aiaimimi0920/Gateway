mod bootstrap;
mod proof;
mod request;
mod response;

pub use super::shared::{
    CHATGPT_WEB_CHAT_FAMILY, CHATGPT_WEB_REVERSE_ADAPTER, CHATGPT_WEB_REVERSE_PROFILE,
};
pub use bootstrap::{
    bootstrap_from_payload_cache, merge_bootstrap_from_fallback, normalize_site_base_url,
    parse_bootstrap_from_html,
};
pub use proof::{build_legacy_requirements_token, build_proof_token, solve_turnstile_token};
pub use request::{pack_request, supports_endpoint};
pub use response::{
    accumulate_response, classify_chatgpt_web_http_error, response_indicates_browser_challenge,
    response_indicates_session_invalid, translate_chatgpt_web_stream, translate_to_openai_sse,
};

pub const CHATGPT_WEB_DEFAULT_BOOTSTRAP_PATH: &str = "/";
pub const CHATGPT_WEB_DEFAULT_REQUIREMENTS_PATH: &str = "/backend-api/sentinel/chat-requirements";
pub const CHATGPT_WEB_DEFAULT_CONVERSATION_PATH: &str = "/backend-api/conversation";
pub const CHATGPT_WEB_DEFAULT_F_CONVERSATION_PREPARE_PATH: &str =
    "/backend-api/f/conversation/prepare";
pub const CHATGPT_WEB_DEFAULT_F_CONVERSATION_PATH: &str = "/backend-api/f/conversation";
pub const CHATGPT_WEB_DEFAULT_MODELS_PATH: &str =
    "/backend-api/models?history_and_training_disabled=false";
pub const CHATGPT_WEB_DEFAULT_MODELS_ROUTE: &str = "/backend-api/models";
pub const CHATGPT_WEB_BROWSER_CHALLENGE_REQUIRED_CODE: &str =
    "chatgpt_web_browser_challenge_required";
pub const CHATGPT_WEB_SESSION_INVALID_CODE: &str = "chatgpt_web_session_invalid";
pub const CHATGPT_WEB_BOOTSTRAP_FAILED_CODE: &str = "chatgpt_web_bootstrap_failed";
pub const CHATGPT_WEB_DEFAULT_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/145.0.0.0 Safari/537.36";
pub const CHATGPT_WEB_DEFAULT_SEC_CH_UA: &str =
    "\"Not:A-Brand\";v=\"99\", \"Google Chrome\";v=\"145\", \"Chromium\";v=\"145\"";
pub const CHATGPT_WEB_DEFAULT_SEC_CH_UA_FULL_VERSION: &str = "\"145.0.0.0\"";
pub const CHATGPT_WEB_DEFAULT_SEC_CH_UA_FULL_VERSION_LIST: &str = "\"Not:A-Brand\";v=\"99.0.0.0\", \"Google Chrome\";v=\"145.0.0.0\", \"Chromium\";v=\"145.0.0.0\"";
pub const CHATGPT_WEB_DEFAULT_SEC_CH_UA_MOBILE: &str = "?0";
pub const CHATGPT_WEB_DEFAULT_SEC_CH_UA_PLATFORM: &str = "\"Windows\"";
pub const CHATGPT_WEB_DEFAULT_SEC_CH_UA_PLATFORM_VERSION: &str = "\"19.0.0\"";
pub const CHATGPT_WEB_DEFAULT_SEC_CH_UA_ARCH: &str = "\"x86\"";
pub const CHATGPT_WEB_DEFAULT_SEC_CH_UA_BITNESS: &str = "\"64\"";
pub const CHATGPT_WEB_DEFAULT_ACCEPT_LANGUAGE: &str = "zh-CN,zh;q=0.9,en;q=0.8,en-US;q=0.7";
pub const CHATGPT_WEB_DEFAULT_LANGUAGE: &str = "zh-CN";
pub const CHATGPT_WEB_DEFAULT_CLIENT_VERSION: &str =
    "prod-be885abbfcfe7b1f511e88b3003d9ee44757fbad";
pub const CHATGPT_WEB_DEFAULT_CLIENT_BUILD_NUMBER: &str = "5955942";
pub const CHATGPT_WEB_DEFAULT_TIMEZONE: &str = "Asia/Shanghai";
pub const CHATGPT_WEB_DEFAULT_TIMEZONE_OFFSET_MIN: i64 = -480;
pub const CHATGPT_WEB_DEFAULT_POW_SCRIPT: &str = "https://chatgpt.com/backend-api/sentinel/sdk.js";

#[derive(Debug, Clone, Default)]
pub struct ChatGptWebBootstrap {
    pub pow_script_sources: Vec<String>,
    pub pow_data_build: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ChatRequirements {
    pub token: String,
    pub proof_token: Option<String>,
    pub turnstile_token: Option<String>,
    pub so_token: Option<String>,
}
