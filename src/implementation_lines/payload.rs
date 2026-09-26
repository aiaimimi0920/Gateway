use super::catalog::RefactoredImplementationLine;
use super::profiles::line_for_adapter;
use crate::routing::candidate::ProviderAccountPayload;

pub fn line_for_payload(payload: &ProviderAccountPayload) -> Option<RefactoredImplementationLine> {
    if let Some(line) = line_for_adapter(payload.adapter.as_str()) {
        return Some(line);
    }
    match payload.adapter.trim() {
        "search_api_compatible" | "linkup_compatible" => {
            let base_url = payload.base_url.to_ascii_lowercase();
            if base_url.contains("api.perplexity.ai") {
                Some(RefactoredImplementationLine::PerplexitySearchOfficialVendorApi)
            } else if base_url.contains("api.tavily.com") {
                Some(RefactoredImplementationLine::TavilySearchOfficialVendorApi)
            } else if base_url.contains("api.exa.ai") {
                Some(RefactoredImplementationLine::ExaSearchOfficialVendorApi)
            } else if base_url.contains("api.jina.ai")
                || base_url.contains("s.jina.ai")
                || base_url.contains("r.jina.ai")
            {
                if base_url.contains("reader") || base_url.contains("r.jina.ai") {
                    Some(RefactoredImplementationLine::JinaReaderOfficialVendorApi)
                } else {
                    Some(RefactoredImplementationLine::JinaSearchOfficialVendorApi)
                }
            } else if base_url.contains("api.linkup.so") {
                Some(RefactoredImplementationLine::LinkupSearchOfficialVendorApi)
            } else if base_url.contains("api.ydc-index.io") || base_url.contains("you.com") {
                Some(RefactoredImplementationLine::YouSearchOfficialVendorApi)
            } else if base_url.contains("websearchapi") {
                Some(RefactoredImplementationLine::WebSearchApiSearchOfficialVendorApi)
            } else {
                None
            }
        }
        "gemini_api_compatible" | "gemini_api_modular_compatible" => {
            if payload.base_url.contains("aiplatform.googleapis.com")
                || payload.base_url.contains("vertexai.googleapis.com")
                || payload.base_url.contains("/publishers/google/")
            {
                Some(RefactoredImplementationLine::GoogleAgentPlatformOfficial)
            } else {
                Some(RefactoredImplementationLine::AIStudioOfficial)
            }
        }
        "openai_compatible" => {
            let base_url = payload.base_url.to_ascii_lowercase();
            if base_url.contains("openai.azure.com")
                || (base_url.contains("azure.com") && base_url.contains("/openai/"))
                || base_url.contains(".cognitiveservices.azure.com")
            {
                Some(RefactoredImplementationLine::AzureOpenAIOfficialVendorApi)
            } else if base_url.contains("chatgpt.com/backend-api/codex") {
                Some(RefactoredImplementationLine::ChatGptCodexOAuthOfficial)
            } else if base_url.contains("api.openai.com") {
                Some(RefactoredImplementationLine::ChatGptOfficialApi)
            } else if base_url.contains("dashscope.aliyuncs.com/compatible-mode/")
                || base_url.contains("coding.dashscope.aliyuncs.com/v1")
            {
                Some(RefactoredImplementationLine::QwenOfficialApi)
            } else if base_url.contains("api.groq.com") || base_url.contains("console.groq.com") {
                Some(RefactoredImplementationLine::GroqOpenAiOfficialVendorApi)
            } else if base_url.contains("integrate.api.nvidia.com")
                || base_url.contains("api.nvidia.com")
            {
                Some(RefactoredImplementationLine::NvidiaOpenAiOfficialVendorApi)
            } else if base_url.contains("api.together.xyz") || base_url.contains("api.together.ai")
            {
                Some(RefactoredImplementationLine::TogetherOpenAiAggregatorApi)
            } else if base_url.contains("openrouter.ai") {
                Some(RefactoredImplementationLine::OpenRouterOpenAiAggregatorApi)
            } else if base_url.contains("muyuan.do") {
                Some(RefactoredImplementationLine::MuyuanOpenAiAggregatorApi)
            } else if base_url.contains("api.poe.com") {
                Some(RefactoredImplementationLine::PoeOpenAiAggregatorApi)
            } else if base_url.contains("api.longcat.chat") {
                Some(RefactoredImplementationLine::LongCatOpenAiOfficialModelApi)
            } else if base_url.contains("api.deepseek.com") {
                Some(RefactoredImplementationLine::DeepSeekOpenAiOfficialModelApi)
            } else if base_url.contains("api.mistral.ai") {
                Some(RefactoredImplementationLine::MistralOpenAiOfficialModelApi)
            } else if base_url.contains("api.x.ai") {
                Some(RefactoredImplementationLine::XaiOpenAiOfficialVendorApi)
            } else if base_url.contains("api.perplexity.ai") {
                Some(RefactoredImplementationLine::PerplexityChatOfficialVendorApi)
            } else if base_url.contains("xf-yun.com") || base_url.contains("xfyun.cn") {
                Some(RefactoredImplementationLine::XfyunOpenAiOfficialVendorApi)
            } else {
                None
            }
        }
        "anthropic_compatible" => {
            let base_url = payload.base_url.to_ascii_lowercase();
            if base_url.contains("coding.dashscope.aliyuncs.com/apps/anthropic") {
                Some(RefactoredImplementationLine::QwenOfficialApi)
            } else if base_url.contains("api.anthropic.com") {
                Some(RefactoredImplementationLine::AnthropicMessagesOfficialModelApi)
            } else {
                None
            }
        }
        "bedrock_converse_compatible" => {
            Some(RefactoredImplementationLine::AwsBedrockConverseOfficialModelApi)
        }
        "cohere_compatible" => Some(RefactoredImplementationLine::CohereChatOfficialModelApi),
        "grok_compatible" => Some(RefactoredImplementationLine::GrokWebReverseApi),
        "suno_compatible" => Some(RefactoredImplementationLine::SunoWebReverseApi),
        "udio_compatible" => Some(RefactoredImplementationLine::UdioWebReverseApi),
        "lumalabs_compatible" => Some(RefactoredImplementationLine::LumaLabsWebReverseApi),
        "freebuff_compatible" => Some(RefactoredImplementationLine::FreeBuffWebReverseApi),
        "xfyun_websocket_compatible" => {
            Some(RefactoredImplementationLine::XfyunNativeWebSocketOfficialVendorApi)
        }
        "producer_compatible" => Some(RefactoredImplementationLine::ProducerWebReverseApi),
        _ => None,
    }
}
