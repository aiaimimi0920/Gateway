use crate::error::GatewayError;
use crate::routing::candidate::ProviderAccountPayload;

mod catalog;
mod payload;
mod profiles;

pub use catalog::{
    RefactoredImplementationLine, FAMILY_ANTHROPIC_COMPATIBLE_OFFICIAL_API_FEATURE,
    FAMILY_BEDROCK_CONVERSE_OFFICIAL_API_FEATURE, FAMILY_COHERE_CHAT_OFFICIAL_API_FEATURE,
    FAMILY_OPENAI_COMPATIBLE_OFFICIAL_API_FEATURE,
    FAMILY_SEARCH_API_COMPATIBLE_OFFICIAL_API_FEATURE, LINE_AISTUDIO_OFFICIAL_FEATURE,
    LINE_AISTUDIO_WEB_REVERSE_FEATURE, LINE_ANTHROPIC_MESSAGES_OFFICIAL_MODEL_API_FEATURE,
    LINE_AWS_BEDROCK_CONVERSE_OFFICIAL_MODEL_API_FEATURE,
    LINE_AZURE_OPENAI_OFFICIAL_VENDOR_API_FEATURE, LINE_CHATAIBOT_WEB_REVERSE_FEATURE,
    LINE_CHATGPT_CODEX_OAUTH_OFFICIAL_FEATURE, LINE_CHATGPT_OFFICIAL_API_FEATURE,
    LINE_CHATGPT_WEB_REVERSE_FEATURE, LINE_COHERE_CHAT_OFFICIAL_MODEL_API_FEATURE,
    LINE_DEEPSEEK_OPENAI_OFFICIAL_MODEL_API_FEATURE, LINE_EXA_SEARCH_OFFICIAL_VENDOR_API_FEATURE,
    LINE_FREEBUFF_WEB_REVERSE_API_FEATURE, LINE_GEMINI_CANVAS_PROGRAM_FEATURE,
    LINE_GEMINI_WEB_REVERSE_FEATURE, LINE_GOOGLE_AGENT_PLATFORM_OFFICIAL_FEATURE,
    LINE_GROK_WEB_REVERSE_API_FEATURE, LINE_GROQ_OPENAI_OFFICIAL_VENDOR_API_FEATURE,
    LINE_JINA_READER_OFFICIAL_VENDOR_API_FEATURE, LINE_JINA_SEARCH_OFFICIAL_VENDOR_API_FEATURE,
    LINE_KIRO_OFFICIAL_VENDOR_API_FEATURE, LINE_LINKUP_SEARCH_OFFICIAL_VENDOR_API_FEATURE,
    LINE_LONGCAT_OPENAI_OFFICIAL_MODEL_API_FEATURE, LINE_LUMALABS_WEB_REVERSE_API_FEATURE,
    LINE_MISTRAL_OPENAI_OFFICIAL_MODEL_API_FEATURE, LINE_MUYUAN_OPENAI_AGGREGATOR_API_FEATURE,
    LINE_NVIDIA_OPENAI_OFFICIAL_VENDOR_API_FEATURE, LINE_OPENROUTER_OPENAI_AGGREGATOR_API_FEATURE,
    LINE_PERPLEXITY_CHAT_OFFICIAL_VENDOR_API_FEATURE,
    LINE_PERPLEXITY_SEARCH_OFFICIAL_VENDOR_API_FEATURE, LINE_POE_OPENAI_AGGREGATOR_API_FEATURE,
    LINE_PRODUCER_WEB_REVERSE_API_FEATURE, LINE_QWEN_OFFICIAL_API_FEATURE,
    LINE_QWEN_WEB_REVERSE_FEATURE, LINE_SUNO_WEB_REVERSE_API_FEATURE,
    LINE_TAVILY_SEARCH_OFFICIAL_VENDOR_API_FEATURE, LINE_TOGETHER_OPENAI_AGGREGATOR_API_FEATURE,
    LINE_UDIO_WEB_REVERSE_API_FEATURE, LINE_WEBSEARCHAPI_SEARCH_OFFICIAL_VENDOR_API_FEATURE,
    LINE_XAI_OPENAI_OFFICIAL_VENDOR_API_FEATURE,
    LINE_XFYUN_NATIVE_WEBSOCKET_OFFICIAL_VENDOR_API_FEATURE,
    LINE_XFYUN_OPENAI_OFFICIAL_VENDOR_API_FEATURE, LINE_YOU_SEARCH_OFFICIAL_VENDOR_API_FEATURE,
};
pub use payload::line_for_payload;
pub use profiles::{line_for_adapter, line_for_protocol_profile};

pub fn accio_web_reverse_api_compiled() -> bool {
    cfg!(feature = "line-accio-web-reverse-api")
}

pub fn accio_web_reverse_api_compiled_out_error(context: &str) -> GatewayError {
    GatewayError::service_unavailable(format!(
        "Accio web_reverse_api line is not compiled into this gateway binary; {context}"
    ))
    .with_code("gateway_provider_line_compiled_out")
}

pub fn required_feature_for_protocol_profile(profile: &str) -> Option<&'static str> {
    line_for_protocol_profile(profile).map(RefactoredImplementationLine::feature_name)
}

pub fn required_feature_for_adapter(adapter: &str) -> Option<&'static str> {
    line_for_adapter(adapter).map(RefactoredImplementationLine::feature_name)
}

pub fn is_protocol_profile_compiled_in(profile: &str) -> bool {
    line_for_protocol_profile(profile)
        .map(RefactoredImplementationLine::compiled_in)
        .unwrap_or(true)
}

pub fn is_adapter_compiled_in(adapter: &str) -> bool {
    line_for_adapter(adapter)
        .map(RefactoredImplementationLine::compiled_in)
        .unwrap_or(true)
}

pub fn ensure_protocol_profile_compiled(profile: &str) -> Result<(), GatewayError> {
    let Some(line) = line_for_protocol_profile(profile) else {
        return Ok(());
    };
    if line.compiled_in() {
        return Ok(());
    }
    Err(compiled_out_error_for_line(line))
}

pub fn ensure_payload_compiled(payload: &ProviderAccountPayload) -> Result<(), GatewayError> {
    let Some(line) = line_for_payload(payload) else {
        return Ok(());
    };
    if line.compiled_in() {
        return Ok(());
    }
    Err(compiled_out_error_for_line(line).with_provider(payload.adapter.clone()))
}

pub fn assert_adapter_compiled(adapter: &str, context: &str) -> Result<(), GatewayError> {
    let Some(line) = line_for_adapter(adapter) else {
        return Ok(());
    };
    if line.compiled_in() {
        return Ok(());
    }
    let mut error = compiled_out_error_for_line(line).with_provider(adapter.trim().to_string());
    error.message = format!("{}; {context}", error.message);
    Err(error)
}

pub fn compiled_out_error_for_line(line: RefactoredImplementationLine) -> GatewayError {
    GatewayError::conflict(format!(
        "Implementation line '{}' was compiled out. Rebuild gateway with cargo feature '{}'.",
        line.canonical_protocol_profile(),
        line.feature_name()
    ))
    .with_code("gateway_provider_line_compiled_out")
}

pub fn search_api_family_compiled_out_error(context: &str) -> GatewayError {
    GatewayError::conflict(format!(
        "Search API family was compiled out. Rebuild gateway with cargo feature '{}'; {context}",
        FAMILY_SEARCH_API_COMPATIBLE_OFFICIAL_API_FEATURE
    ))
    .with_code("gateway_provider_line_compiled_out")
}

pub fn assert_accio_web_reverse_api_compiled(context: &str) -> Result<(), GatewayError> {
    if accio_web_reverse_api_compiled() {
        Ok(())
    } else {
        Err(accio_web_reverse_api_compiled_out_error(context))
    }
}

#[cfg(test)]
mod tests;
