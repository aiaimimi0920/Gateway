use crate::protocol::canonical::EndpointKind;
use crate::protocol::registry::canonicalize_protocol_profile_key;
use crate::routing::candidate::ProviderAccountPayload;

pub const QWEN_OFFICIAL_API_IMPLEMENTATION_LINE: &str = "qwen_official_api";
pub const QWEN_DASHSCOPE_OPENAI_PROFILE: &str = "qwen_dashscope_openai";
pub const QWEN_DASHSCOPE_OPENAI_PRESET_ID: &str = "qwen-dashscope-openai";
pub const QWEN_DASHSCOPE_OPENAI_CATALOG_KEY: &str = "qwen-dashscope-openai";
pub const QWEN_CODING_PLAN_OPENAI_PROFILE: &str = "qwen_coding_plan_openai";
pub const QWEN_CODING_PLAN_OPENAI_PRESET_ID: &str = "qwen-coding-plan-openai";
pub const QWEN_CODING_PLAN_OPENAI_CATALOG_KEY: &str = "qwen-coding-plan-openai";
pub const QWEN_CODING_PLAN_ANTHROPIC_PROFILE: &str = "qwen_coding_plan_anthropic";
pub const QWEN_CODING_PLAN_ANTHROPIC_PRESET_ID: &str = "qwen-coding-plan-anthropic";
pub const QWEN_CODING_PLAN_ANTHROPIC_CATALOG_KEY: &str = "qwen-coding-plan-anthropic";

pub fn is_qwen_official_api_profile(value: &str) -> bool {
    matches!(
        canonicalize_protocol_profile_key(value).as_str(),
        QWEN_DASHSCOPE_OPENAI_PROFILE
            | QWEN_CODING_PLAN_OPENAI_PROFILE
            | QWEN_CODING_PLAN_ANTHROPIC_PROFILE
    )
}

pub fn is_qwen_dashscope_openai_base_url(base_url: &str) -> bool {
    let normalized = base_url.trim_end_matches('/').to_ascii_lowercase();
    normalized.contains("dashscope.aliyuncs.com/compatible-mode/")
}

pub fn is_qwen_coding_plan_openai_base_url(base_url: &str) -> bool {
    let normalized = base_url.trim_end_matches('/').to_ascii_lowercase();
    normalized.contains("coding.dashscope.aliyuncs.com/v1")
        && !normalized.contains("/apps/anthropic")
}

pub fn is_qwen_coding_plan_anthropic_base_url(base_url: &str) -> bool {
    base_url
        .trim_end_matches('/')
        .to_ascii_lowercase()
        .contains("coding.dashscope.aliyuncs.com/apps/anthropic")
}

pub fn infer_qwen_official_profile_from_payload(
    payload: &ProviderAccountPayload,
) -> Option<&'static str> {
    match payload.adapter.trim() {
        "openai_compatible" if is_qwen_dashscope_openai_base_url(&payload.base_url) => {
            Some(QWEN_DASHSCOPE_OPENAI_PROFILE)
        }
        "openai_compatible" if is_qwen_coding_plan_openai_base_url(&payload.base_url) => {
            Some(QWEN_CODING_PLAN_OPENAI_PROFILE)
        }
        "anthropic_compatible" if is_qwen_coding_plan_anthropic_base_url(&payload.base_url) => {
            Some(QWEN_CODING_PLAN_ANTHROPIC_PROFILE)
        }
        _ => None,
    }
}

pub fn owns_payload(payload: &ProviderAccountPayload) -> bool {
    infer_qwen_official_profile_from_payload(payload).is_some()
}

pub fn provider_line_name(_payload: &ProviderAccountPayload) -> &'static str {
    QWEN_OFFICIAL_API_IMPLEMENTATION_LINE
}

pub fn supports_endpoint(endpoint_kind: EndpointKind) -> bool {
    matches!(
        endpoint_kind,
        EndpointKind::ChatCompletions
            | EndpointKind::Responses
            | EndpointKind::Messages
            | EndpointKind::Completions
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::routing::candidate::ProviderAccountPayload;
    use std::collections::HashMap;

    fn make_payload(adapter: &str, base_url: &str) -> ProviderAccountPayload {
        ProviderAccountPayload {
            discovered_protocols: Vec::new(),
            adapter: adapter.to_string(),
            base_url: base_url.to_string(),
            api_key: "tok".to_string(),
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

    #[test]
    fn recognizes_qwen_official_profiles_and_base_urls() {
        assert!(is_qwen_official_api_profile("qwen"));
        assert!(is_qwen_official_api_profile("qwen_dashscope_openai"));
        assert!(is_qwen_official_api_profile("qwen_coding_plan_openai"));
        assert!(is_qwen_official_api_profile("qwen_coding_plan_anthropic"));
        assert!(is_qwen_dashscope_openai_base_url(
            "https://dashscope.aliyuncs.com/compatible-mode/v1"
        ));
        assert!(is_qwen_coding_plan_openai_base_url(
            "https://coding.dashscope.aliyuncs.com/v1"
        ));
        assert!(is_qwen_coding_plan_anthropic_base_url(
            "https://coding.dashscope.aliyuncs.com/apps/anthropic"
        ));
    }

    #[test]
    fn infers_qwen_official_surface_from_payload() {
        assert_eq!(
            infer_qwen_official_profile_from_payload(&make_payload(
                "openai_compatible",
                "https://dashscope.aliyuncs.com/compatible-mode/v1"
            )),
            Some(QWEN_DASHSCOPE_OPENAI_PROFILE)
        );
        assert_eq!(
            infer_qwen_official_profile_from_payload(&make_payload(
                "openai_compatible",
                "https://coding.dashscope.aliyuncs.com/v1"
            )),
            Some(QWEN_CODING_PLAN_OPENAI_PROFILE)
        );
        assert_eq!(
            infer_qwen_official_profile_from_payload(&make_payload(
                "anthropic_compatible",
                "https://coding.dashscope.aliyuncs.com/apps/anthropic"
            )),
            Some(QWEN_CODING_PLAN_ANTHROPIC_PROFILE)
        );
        assert!(owns_payload(&make_payload(
            "openai_compatible",
            "https://coding.dashscope.aliyuncs.com/v1"
        )));
    }
}
