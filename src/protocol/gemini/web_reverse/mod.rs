use crate::protocol::canonical::EndpointKind;

mod bootstrap;
mod constants;
mod headers;
mod media;
mod request;
mod response;
mod types;

pub use super::shared::{GEMINI_WEB_REVERSE_MODULAR_ADAPTER, GEMINI_WEB_REVERSE_MODULAR_PROFILE};
pub use bootstrap::{
    bootstrap_from_payload_cache, extract_app_page_path_from_html, extract_app_page_path_from_url,
    merge_bootstrap_from_fallback, normalize_app_page_path_candidate,
    parse_bootstrap_from_app_html,
};
pub use constants::{
    GEMINI_WEB_BOOTSTRAP_FAILED_CODE, GEMINI_WEB_BROWSER_CHALLENGE_REQUIRED_CODE,
    GEMINI_WEB_DEFAULT_ACCEPT_LANGUAGE, GEMINI_WEB_DEFAULT_APP_PATH,
    GEMINI_WEB_DEFAULT_SAME_DOMAIN_HEADER, GEMINI_WEB_DEFAULT_SEC_CH_UA,
    GEMINI_WEB_DEFAULT_SEC_CH_UA_ARCH, GEMINI_WEB_DEFAULT_SEC_CH_UA_BITNESS,
    GEMINI_WEB_DEFAULT_SEC_CH_UA_FORM_FACTORS, GEMINI_WEB_DEFAULT_SEC_CH_UA_FULL_VERSION,
    GEMINI_WEB_DEFAULT_SEC_CH_UA_FULL_VERSION_LIST, GEMINI_WEB_DEFAULT_SEC_CH_UA_MOBILE,
    GEMINI_WEB_DEFAULT_SEC_CH_UA_MODEL, GEMINI_WEB_DEFAULT_SEC_CH_UA_PLATFORM,
    GEMINI_WEB_DEFAULT_SEC_CH_UA_PLATFORM_VERSION, GEMINI_WEB_DEFAULT_SEC_CH_UA_WOW64,
    GEMINI_WEB_DEFAULT_STREAM_GENERATE_PATH, GEMINI_WEB_DEFAULT_USER_AGENT,
    GEMINI_WEB_MODEL_HEADER_2_KEY, GEMINI_WEB_MODEL_HEADER_3_KEY, GEMINI_WEB_MODEL_HEADER_KEY,
    GEMINI_WEB_REQUEST_CONTEXT_HEADER_KEY, GEMINI_WEB_SESSION_INVALID_CODE,
};
pub use headers::extract_model_headers;
pub use media::prompt_for_legacy_mixed_lane_media_request;
pub use request::pack_request;
pub use response::{
    accumulate_gemini_web_response, classify_gemini_web_http_error, parse_response,
    response_indicates_browser_challenge, response_indicates_session_invalid,
    translate_gemini_web_to_openai_sse,
};
pub use types::{GeminiWebBootstrap, GeminiWebRequest};

pub fn supports_endpoint(endpoint_kind: EndpointKind) -> bool {
    matches!(
        endpoint_kind,
        EndpointKind::ChatCompletions
            | EndpointKind::Messages
            | EndpointKind::Responses
            | EndpointKind::Completions
    )
}
