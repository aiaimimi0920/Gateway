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
