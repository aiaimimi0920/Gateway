use bytes::Bytes;
use serde_json::Value;
use std::collections::HashMap;

use crate::error::GatewayError;
use crate::protocol::canonical::{CanonicalRelayRequest, CanonicalRelayResponse};
use crate::protocol::gemini::web_reverse as modular;

pub use crate::protocol::gemini::web_reverse::{
    GeminiWebBootstrap, GeminiWebRequest, GEMINI_WEB_BOOTSTRAP_FAILED_CODE,
    GEMINI_WEB_BROWSER_CHALLENGE_REQUIRED_CODE, GEMINI_WEB_DEFAULT_ACCEPT_LANGUAGE,
    GEMINI_WEB_DEFAULT_APP_PATH, GEMINI_WEB_DEFAULT_SAME_DOMAIN_HEADER,
    GEMINI_WEB_DEFAULT_SEC_CH_UA, GEMINI_WEB_DEFAULT_SEC_CH_UA_ARCH,
    GEMINI_WEB_DEFAULT_SEC_CH_UA_BITNESS, GEMINI_WEB_DEFAULT_SEC_CH_UA_FORM_FACTORS,
    GEMINI_WEB_DEFAULT_SEC_CH_UA_FULL_VERSION, GEMINI_WEB_DEFAULT_SEC_CH_UA_FULL_VERSION_LIST,
    GEMINI_WEB_DEFAULT_SEC_CH_UA_MOBILE, GEMINI_WEB_DEFAULT_SEC_CH_UA_MODEL,
    GEMINI_WEB_DEFAULT_SEC_CH_UA_PLATFORM, GEMINI_WEB_DEFAULT_SEC_CH_UA_PLATFORM_VERSION,
    GEMINI_WEB_DEFAULT_SEC_CH_UA_WOW64, GEMINI_WEB_DEFAULT_STREAM_GENERATE_PATH,
    GEMINI_WEB_DEFAULT_USER_AGENT, GEMINI_WEB_MODEL_HEADER_2_KEY, GEMINI_WEB_MODEL_HEADER_3_KEY,
    GEMINI_WEB_MODEL_HEADER_KEY, GEMINI_WEB_REQUEST_CONTEXT_HEADER_KEY,
    GEMINI_WEB_SESSION_INVALID_CODE,
};

pub fn unsupported_request_plan_error() -> GatewayError {
    GatewayError::bad_request(
        "Gemini Web adapters use an app bootstrap + StreamGenerate form flow and are not supported by the generic request planner.",
    )
    .with_code("unsupported_gemini_web_request_plan")
}

pub fn pack_gemini_web(
    req: &CanonicalRelayRequest,
    model: &str,
    bootstrap: &GeminiWebBootstrap,
) -> Result<GeminiWebRequest, GatewayError> {
    modular::pack_request(req, model, bootstrap)
}

pub fn parse_bootstrap_from_app_html(
    html: &str,
    fallback_language: Option<&str>,
) -> Result<GeminiWebBootstrap, GatewayError> {
    modular::parse_bootstrap_from_app_html(html, fallback_language)
}

pub fn bootstrap_from_payload_cache(
    extra_body: Option<&HashMap<String, Value>>,
) -> Option<GeminiWebBootstrap> {
    modular::bootstrap_from_payload_cache(extra_body)
}

pub fn merge_bootstrap_from_fallback(
    primary: GeminiWebBootstrap,
    fallback: Option<&GeminiWebBootstrap>,
) -> GeminiWebBootstrap {
    modular::merge_bootstrap_from_fallback(primary, fallback)
}

pub fn normalize_app_page_path_candidate(candidate: &str) -> Option<String> {
    modular::normalize_app_page_path_candidate(candidate)
}

pub fn extract_app_page_path_from_html(html: &str) -> Option<String> {
    modular::extract_app_page_path_from_html(html)
}

pub fn extract_app_page_path_from_url(url: &str) -> Option<String> {
    modular::extract_app_page_path_from_url(url)
}

pub fn extract_model_headers(
    extra_body: Option<&HashMap<String, Value>>,
    model: &str,
) -> Vec<(String, String)> {
    modular::extract_model_headers(extra_body, model)
}

pub fn classify_gemini_web_http_error(
    status: u16,
    content_type: Option<&str>,
    body: &str,
) -> GatewayError {
    modular::classify_gemini_web_http_error(status, content_type, body)
}

pub fn response_indicates_browser_challenge(
    status: u16,
    content_type: Option<&str>,
    body: &str,
) -> bool {
    modular::response_indicates_browser_challenge(status, content_type, body)
}

pub fn response_indicates_session_invalid(
    status: u16,
    content_type: Option<&str>,
    body: &str,
) -> bool {
    modular::response_indicates_session_invalid(status, content_type, body)
}

pub fn accumulate_gemini_web_response(
    body: &str,
    model: &str,
) -> Result<CanonicalRelayResponse, GatewayError> {
    modular::accumulate_gemini_web_response(body, model)
}

pub fn translate_gemini_web_to_openai_sse(
    body: &str,
    model: &str,
) -> Result<Vec<Bytes>, GatewayError> {
    modular::translate_gemini_web_to_openai_sse(body, model)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use serde_json::json;

    use super::unsupported_request_plan_error;
    use crate::protocol::canonical::{CanonicalRelayRequest, EndpointKind, ProtocolFamily};
    use crate::routing::candidate::ProviderAccountPayload;
    use crate::upstream::client::UpstreamClient;

    fn make_payload(adapter: &str, base_url: &str) -> ProviderAccountPayload {
        ProviderAccountPayload {
            discovered_protocols: Vec::new(),
            adapter: adapter.to_string(),
            base_url: base_url.to_string(),
            api_key: "sk-test".to_string(),
            credential_id: None,
            expires_at: None,
            runtime_state_object_key: None,
            account_name: None,
            execution_mode: None,
            endpoint_execution_modes: None,
            default_model: None,
            headers: HashMap::new(),
            auth_mode: None,
            anthropic_version: None,
            beta_headers: None,
            auth_header_name: None,
            auth_token: None,
            responses_path: None,
            chat_completions_path: None,
            completions_path: None,
            embeddings_path: None,
            audio_transcriptions_path: None,
            audio_speech_path: None,
            messages_path: None,
            search_path: None,
            fetch_path: None,
            research_path: None,
            balance_path: None,
            search_query_field: None,
            fetch_urls_field: None,
            extra_body: None,
            session_auth: None,
            keepalive: None,
        }
    }

    fn make_request(protocol: ProtocolFamily, endpoint: EndpointKind) -> CanonicalRelayRequest {
        CanonicalRelayRequest {
            protocol_family: protocol,
            endpoint_kind: endpoint,
            requested_model: Some("test-model".to_string()),
            stream: false,
            messages: vec![],
            tools: vec![],
            tool_choice: None,
            reasoning: None,
            metadata: None,
            raw_body: json!({}),
            previous_response_id: None,
            explicit_session_key: None,
            extra: HashMap::new(),
        }
    }

    #[test]
    fn gemini_web_unsupported_request_plan_error_matches_contract() {
        let err = unsupported_request_plan_error();
        assert_eq!(err.http_status, Some(400));
        assert_eq!(
            err.code.as_deref(),
            Some("unsupported_gemini_web_request_plan")
        );
    }

    #[test]
    fn plan_gemini_web_chat_endpoint_rejected_locally() {
        let payload = make_payload("gemini_web_compatible", "https://gemini.google.com");
        let req = make_request(ProtocolFamily::OpenAi, EndpointKind::ChatCompletions);
        let err = UpstreamClient::build_request_plan(&payload, &req, "gemini-2.0-flash-exp", false)
            .expect_err("gemini web chat requests should be rejected");
        assert_eq!(err.http_status, Some(400));
        assert_eq!(
            err.code.as_deref(),
            Some("unsupported_gemini_web_request_plan")
        );
    }
}
