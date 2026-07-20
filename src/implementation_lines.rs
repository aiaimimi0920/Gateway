use crate::error::GatewayError;
use crate::routing::candidate::ProviderAccountPayload;

pub fn accio_web_reverse_api_compiled() -> bool {
    cfg!(feature = "line-accio-web-reverse-api")
}

pub fn accio_web_reverse_api_compiled_out_error(context: &str) -> GatewayError {
    GatewayError::service_unavailable(format!(
        "Accio web_reverse_api line is not compiled into this gateway binary; {context}"
    ))
    .with_code("gateway_provider_line_compiled_out")
}

pub const LINE_GEMINI_WEB_REVERSE_FEATURE: &str = "line-gemini-web-reverse";
pub const LINE_GEMINI_CANVAS_PROGRAM_FEATURE: &str = "line-gemini-canvas-program";
pub const LINE_AISTUDIO_OFFICIAL_FEATURE: &str = "line-aistudio-official";
pub const LINE_AISTUDIO_WEB_REVERSE_FEATURE: &str = "line-aistudio-web-reverse";
pub const LINE_GOOGLE_AGENT_PLATFORM_OFFICIAL_FEATURE: &str = "line-google-agent-platform-official";
pub const LINE_AZURE_OPENAI_OFFICIAL_VENDOR_API_FEATURE: &str =
    "line-azure-openai-official-vendor-api";
pub const LINE_ANTHROPIC_MESSAGES_OFFICIAL_MODEL_API_FEATURE: &str =
    "line-anthropic-messages-official-model-api";
pub const LINE_AWS_BEDROCK_CONVERSE_OFFICIAL_MODEL_API_FEATURE: &str =
    "line-aws-bedrock-converse-official-model-api";
pub const LINE_CHATGPT_OFFICIAL_API_FEATURE: &str = "line-chatgpt-official-api";
pub const LINE_CHATGPT_CODEX_OAUTH_OFFICIAL_FEATURE: &str = "line-chatgpt-codex-oauth-official";
pub const LINE_CHATGPT_WEB_REVERSE_FEATURE: &str = "line-chatgpt-web-reverse";
pub const LINE_COHERE_CHAT_OFFICIAL_MODEL_API_FEATURE: &str = "line-cohere-chat-official-model-api";
pub const LINE_GROQ_OPENAI_OFFICIAL_VENDOR_API_FEATURE: &str =
    "line-groq-openai-official-vendor-api";
pub const LINE_NVIDIA_OPENAI_OFFICIAL_VENDOR_API_FEATURE: &str =
    "line-nvidia-openai-official-vendor-api";
pub const LINE_TOGETHER_OPENAI_AGGREGATOR_API_FEATURE: &str = "line-together-openai-aggregator-api";
pub const LINE_OPENROUTER_OPENAI_AGGREGATOR_API_FEATURE: &str =
    "line-openrouter-openai-aggregator-api";
pub const LINE_DEEPSEEK_OPENAI_OFFICIAL_MODEL_API_FEATURE: &str =
    "line-deepseek-openai-official-model-api";
pub const LINE_MISTRAL_OPENAI_OFFICIAL_MODEL_API_FEATURE: &str =
    "line-mistral-openai-official-model-api";
pub const LINE_XAI_OPENAI_OFFICIAL_VENDOR_API_FEATURE: &str = "line-xai-openai-official-vendor-api";
pub const LINE_PERPLEXITY_CHAT_OFFICIAL_VENDOR_API_FEATURE: &str =
    "line-perplexity-chat-official-vendor-api";
pub const LINE_XFYUN_OPENAI_OFFICIAL_VENDOR_API_FEATURE: &str =
    "line-xfyun-openai-official-vendor-api";
pub const LINE_XFYUN_NATIVE_WEBSOCKET_OFFICIAL_VENDOR_API_FEATURE: &str =
    "line-xfyun-native-websocket-official-vendor-api";
pub const LINE_FREEBUFF_WEB_REVERSE_API_FEATURE: &str = "line-freebuff-web-reverse-api";
pub const LINE_PRODUCER_WEB_REVERSE_API_FEATURE: &str = "line-producer-web-reverse-api";
pub const LINE_GROK_WEB_REVERSE_API_FEATURE: &str = "line-grok-web-reverse-api";
pub const LINE_SUNO_WEB_REVERSE_API_FEATURE: &str = "line-suno-web-reverse-api";
pub const LINE_UDIO_WEB_REVERSE_API_FEATURE: &str = "line-udio-web-reverse-api";
pub const LINE_LUMALABS_WEB_REVERSE_API_FEATURE: &str = "line-lumalabs-web-reverse-api";
pub const LINE_PERPLEXITY_SEARCH_OFFICIAL_VENDOR_API_FEATURE: &str =
    "line-perplexity-search-official-vendor-api";
pub const LINE_TAVILY_SEARCH_OFFICIAL_VENDOR_API_FEATURE: &str =
    "line-tavily-search-official-vendor-api";
pub const LINE_EXA_SEARCH_OFFICIAL_VENDOR_API_FEATURE: &str = "line-exa-search-official-vendor-api";
pub const LINE_JINA_SEARCH_OFFICIAL_VENDOR_API_FEATURE: &str =
    "line-jina-search-official-vendor-api";
pub const LINE_JINA_READER_OFFICIAL_VENDOR_API_FEATURE: &str =
    "line-jina-reader-official-vendor-api";
pub const LINE_LINKUP_SEARCH_OFFICIAL_VENDOR_API_FEATURE: &str =
    "line-linkup-search-official-vendor-api";
pub const LINE_YOU_SEARCH_OFFICIAL_VENDOR_API_FEATURE: &str = "line-you-search-official-vendor-api";
pub const LINE_WEBSEARCHAPI_SEARCH_OFFICIAL_VENDOR_API_FEATURE: &str =
    "line-websearchapi-search-official-vendor-api";
pub const LINE_QWEN_OFFICIAL_API_FEATURE: &str = "line-qwen-official-api";
pub const LINE_QWEN_WEB_REVERSE_FEATURE: &str = "line-qwen-web-reverse";
pub const LINE_CHATAIBOT_WEB_REVERSE_FEATURE: &str = "line-chataibot-web-reverse";
pub const LINE_KIRO_OFFICIAL_VENDOR_API_FEATURE: &str = "line-kiro-official-vendor-api";
pub const FAMILY_OPENAI_COMPATIBLE_OFFICIAL_API_FEATURE: &str =
    "family-openai-compatible-official-api";
pub const FAMILY_ANTHROPIC_COMPATIBLE_OFFICIAL_API_FEATURE: &str =
    "family-anthropic-compatible-official-api";
pub const FAMILY_BEDROCK_CONVERSE_OFFICIAL_API_FEATURE: &str =
    "family-bedrock-converse-official-api";
pub const FAMILY_COHERE_CHAT_OFFICIAL_API_FEATURE: &str = "family-cohere-chat-official-api";
pub const FAMILY_SEARCH_API_COMPATIBLE_OFFICIAL_API_FEATURE: &str =
    "family-search-api-compatible-official-api";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RefactoredImplementationLine {
    GeminiWebReverse,
    GeminiCanvasProgram,
    AIStudioOfficial,
    AIStudioWebReverse,
    GoogleAgentPlatformOfficial,
    AzureOpenAIOfficialVendorApi,
    AnthropicMessagesOfficialModelApi,
    AwsBedrockConverseOfficialModelApi,
    ChatGptOfficialApi,
    ChatGptCodexOAuthOfficial,
    ChatGptWebReverse,
    CohereChatOfficialModelApi,
    GroqOpenAiOfficialVendorApi,
    NvidiaOpenAiOfficialVendorApi,
    TogetherOpenAiAggregatorApi,
    OpenRouterOpenAiAggregatorApi,
    DeepSeekOpenAiOfficialModelApi,
    MistralOpenAiOfficialModelApi,
    XaiOpenAiOfficialVendorApi,
    PerplexityChatOfficialVendorApi,
    XfyunOpenAiOfficialVendorApi,
    XfyunNativeWebSocketOfficialVendorApi,
    FreeBuffWebReverseApi,
    ProducerWebReverseApi,
    GrokWebReverseApi,
    SunoWebReverseApi,
    UdioWebReverseApi,
    LumaLabsWebReverseApi,
    PerplexitySearchOfficialVendorApi,
    TavilySearchOfficialVendorApi,
    ExaSearchOfficialVendorApi,
    JinaSearchOfficialVendorApi,
    JinaReaderOfficialVendorApi,
    LinkupSearchOfficialVendorApi,
    YouSearchOfficialVendorApi,
    WebSearchApiSearchOfficialVendorApi,
    QwenOfficialApi,
    QwenWebReverse,
    ChataibotWebReverse,
    KiroOfficialVendorApi,
}

impl RefactoredImplementationLine {
    pub fn canonical_protocol_profile(self) -> &'static str {
        match self {
            Self::GeminiWebReverse => "gemini_web_reverse_modular",
            Self::GeminiCanvasProgram => "gemini_canvas_program_web_reverse_modular",
            Self::AIStudioOfficial => "aistudio_official_api",
            Self::AIStudioWebReverse => "aistudio_web_reverse",
            Self::GoogleAgentPlatformOfficial => "google_agent_platform_official_api",
            Self::AzureOpenAIOfficialVendorApi => "azure_openai",
            Self::AnthropicMessagesOfficialModelApi => "anthropic",
            Self::AwsBedrockConverseOfficialModelApi => "aws_bedrock",
            Self::ChatGptOfficialApi => "chatgpt_official_api",
            Self::ChatGptCodexOAuthOfficial => "chatgpt_codex_oauth_official_api",
            Self::ChatGptWebReverse => "chatgpt_web_reverse",
            Self::CohereChatOfficialModelApi => "cohere",
            Self::GroqOpenAiOfficialVendorApi => "groq",
            Self::NvidiaOpenAiOfficialVendorApi => "nvidia",
            Self::TogetherOpenAiAggregatorApi => "together",
            Self::OpenRouterOpenAiAggregatorApi => "openrouter",
            Self::DeepSeekOpenAiOfficialModelApi => "deepseek",
            Self::MistralOpenAiOfficialModelApi => "mistral",
            Self::XaiOpenAiOfficialVendorApi => "xai",
            Self::PerplexityChatOfficialVendorApi => "perplexity_chat",
            Self::XfyunOpenAiOfficialVendorApi => "xfyun_openai",
            Self::XfyunNativeWebSocketOfficialVendorApi => "xfyun_native_websocket",
            Self::FreeBuffWebReverseApi => "freebuff",
            Self::ProducerWebReverseApi => "producer",
            Self::GrokWebReverseApi => "grok_web",
            Self::SunoWebReverseApi => "suno",
            Self::UdioWebReverseApi => "udio",
            Self::LumaLabsWebReverseApi => "lumalabs",
            Self::PerplexitySearchOfficialVendorApi => "perplexity_search",
            Self::TavilySearchOfficialVendorApi => "tavily",
            Self::ExaSearchOfficialVendorApi => "exa",
            Self::JinaSearchOfficialVendorApi => "jina_search",
            Self::JinaReaderOfficialVendorApi => "jina_reader",
            Self::LinkupSearchOfficialVendorApi => "linkup",
            Self::YouSearchOfficialVendorApi => "you_search",
            Self::WebSearchApiSearchOfficialVendorApi => "websearchapi",
            Self::QwenOfficialApi => "qwen_official_api",
            Self::QwenWebReverse => "qwen_web_reverse",
            Self::ChataibotWebReverse => "chataibot",
            Self::KiroOfficialVendorApi => "kiro",
        }
    }

    pub fn feature_name(self) -> &'static str {
        match self {
            Self::GeminiWebReverse => LINE_GEMINI_WEB_REVERSE_FEATURE,
            Self::GeminiCanvasProgram => LINE_GEMINI_CANVAS_PROGRAM_FEATURE,
            Self::AIStudioOfficial => LINE_AISTUDIO_OFFICIAL_FEATURE,
            Self::AIStudioWebReverse => LINE_AISTUDIO_WEB_REVERSE_FEATURE,
            Self::GoogleAgentPlatformOfficial => LINE_GOOGLE_AGENT_PLATFORM_OFFICIAL_FEATURE,
            Self::AzureOpenAIOfficialVendorApi => LINE_AZURE_OPENAI_OFFICIAL_VENDOR_API_FEATURE,
            Self::AnthropicMessagesOfficialModelApi => {
                LINE_ANTHROPIC_MESSAGES_OFFICIAL_MODEL_API_FEATURE
            }
            Self::AwsBedrockConverseOfficialModelApi => {
                LINE_AWS_BEDROCK_CONVERSE_OFFICIAL_MODEL_API_FEATURE
            }
            Self::ChatGptOfficialApi => LINE_CHATGPT_OFFICIAL_API_FEATURE,
            Self::ChatGptCodexOAuthOfficial => LINE_CHATGPT_CODEX_OAUTH_OFFICIAL_FEATURE,
            Self::ChatGptWebReverse => LINE_CHATGPT_WEB_REVERSE_FEATURE,
            Self::CohereChatOfficialModelApi => LINE_COHERE_CHAT_OFFICIAL_MODEL_API_FEATURE,
            Self::GroqOpenAiOfficialVendorApi => LINE_GROQ_OPENAI_OFFICIAL_VENDOR_API_FEATURE,
            Self::NvidiaOpenAiOfficialVendorApi => LINE_NVIDIA_OPENAI_OFFICIAL_VENDOR_API_FEATURE,
            Self::TogetherOpenAiAggregatorApi => LINE_TOGETHER_OPENAI_AGGREGATOR_API_FEATURE,
            Self::OpenRouterOpenAiAggregatorApi => LINE_OPENROUTER_OPENAI_AGGREGATOR_API_FEATURE,
            Self::DeepSeekOpenAiOfficialModelApi => LINE_DEEPSEEK_OPENAI_OFFICIAL_MODEL_API_FEATURE,
            Self::MistralOpenAiOfficialModelApi => LINE_MISTRAL_OPENAI_OFFICIAL_MODEL_API_FEATURE,
            Self::XaiOpenAiOfficialVendorApi => LINE_XAI_OPENAI_OFFICIAL_VENDOR_API_FEATURE,
            Self::PerplexityChatOfficialVendorApi => {
                LINE_PERPLEXITY_CHAT_OFFICIAL_VENDOR_API_FEATURE
            }
            Self::XfyunOpenAiOfficialVendorApi => LINE_XFYUN_OPENAI_OFFICIAL_VENDOR_API_FEATURE,
            Self::XfyunNativeWebSocketOfficialVendorApi => {
                LINE_XFYUN_NATIVE_WEBSOCKET_OFFICIAL_VENDOR_API_FEATURE
            }
            Self::FreeBuffWebReverseApi => LINE_FREEBUFF_WEB_REVERSE_API_FEATURE,
            Self::ProducerWebReverseApi => LINE_PRODUCER_WEB_REVERSE_API_FEATURE,
            Self::PerplexitySearchOfficialVendorApi => {
                LINE_PERPLEXITY_SEARCH_OFFICIAL_VENDOR_API_FEATURE
            }
            Self::TavilySearchOfficialVendorApi => LINE_TAVILY_SEARCH_OFFICIAL_VENDOR_API_FEATURE,
            Self::ExaSearchOfficialVendorApi => LINE_EXA_SEARCH_OFFICIAL_VENDOR_API_FEATURE,
            Self::JinaSearchOfficialVendorApi => LINE_JINA_SEARCH_OFFICIAL_VENDOR_API_FEATURE,
            Self::JinaReaderOfficialVendorApi => LINE_JINA_READER_OFFICIAL_VENDOR_API_FEATURE,
            Self::LinkupSearchOfficialVendorApi => LINE_LINKUP_SEARCH_OFFICIAL_VENDOR_API_FEATURE,
            Self::YouSearchOfficialVendorApi => LINE_YOU_SEARCH_OFFICIAL_VENDOR_API_FEATURE,
            Self::WebSearchApiSearchOfficialVendorApi => {
                LINE_WEBSEARCHAPI_SEARCH_OFFICIAL_VENDOR_API_FEATURE
            }
            Self::GrokWebReverseApi => LINE_GROK_WEB_REVERSE_API_FEATURE,
            Self::SunoWebReverseApi => LINE_SUNO_WEB_REVERSE_API_FEATURE,
            Self::UdioWebReverseApi => LINE_UDIO_WEB_REVERSE_API_FEATURE,
            Self::LumaLabsWebReverseApi => LINE_LUMALABS_WEB_REVERSE_API_FEATURE,
            Self::QwenOfficialApi => LINE_QWEN_OFFICIAL_API_FEATURE,
            Self::QwenWebReverse => LINE_QWEN_WEB_REVERSE_FEATURE,
            Self::ChataibotWebReverse => LINE_CHATAIBOT_WEB_REVERSE_FEATURE,
            Self::KiroOfficialVendorApi => LINE_KIRO_OFFICIAL_VENDOR_API_FEATURE,
        }
    }

    pub fn compiled_in(self) -> bool {
        match self {
            Self::GeminiWebReverse => cfg!(feature = "line-gemini-web-reverse"),
            Self::GeminiCanvasProgram => cfg!(feature = "line-gemini-canvas-program"),
            Self::AIStudioOfficial => cfg!(feature = "line-aistudio-official"),
            Self::AIStudioWebReverse => cfg!(feature = "line-aistudio-web-reverse"),
            Self::GoogleAgentPlatformOfficial => {
                cfg!(feature = "line-google-agent-platform-official")
            }
            Self::AzureOpenAIOfficialVendorApi => {
                cfg!(feature = "line-azure-openai-official-vendor-api")
            }
            Self::AnthropicMessagesOfficialModelApi => {
                cfg!(feature = "line-anthropic-messages-official-model-api")
            }
            Self::AwsBedrockConverseOfficialModelApi => {
                cfg!(feature = "line-aws-bedrock-converse-official-model-api")
            }
            Self::ChatGptOfficialApi => cfg!(feature = "line-chatgpt-official-api"),
            Self::ChatGptCodexOAuthOfficial => {
                cfg!(feature = "line-chatgpt-codex-oauth-official")
            }
            Self::ChatGptWebReverse => cfg!(feature = "line-chatgpt-web-reverse"),
            Self::CohereChatOfficialModelApi => {
                cfg!(feature = "line-cohere-chat-official-model-api")
            }
            Self::GroqOpenAiOfficialVendorApi => {
                cfg!(feature = "line-groq-openai-official-vendor-api")
            }
            Self::NvidiaOpenAiOfficialVendorApi => {
                cfg!(feature = "line-nvidia-openai-official-vendor-api")
            }
            Self::TogetherOpenAiAggregatorApi => {
                cfg!(feature = "line-together-openai-aggregator-api")
            }
            Self::OpenRouterOpenAiAggregatorApi => {
                cfg!(feature = "line-openrouter-openai-aggregator-api")
            }
            Self::DeepSeekOpenAiOfficialModelApi => {
                cfg!(feature = "line-deepseek-openai-official-model-api")
            }
            Self::MistralOpenAiOfficialModelApi => {
                cfg!(feature = "line-mistral-openai-official-model-api")
            }
            Self::XaiOpenAiOfficialVendorApi => {
                cfg!(feature = "line-xai-openai-official-vendor-api")
            }
            Self::PerplexityChatOfficialVendorApi => {
                cfg!(feature = "line-perplexity-chat-official-vendor-api")
            }
            Self::XfyunOpenAiOfficialVendorApi => {
                cfg!(feature = "line-xfyun-openai-official-vendor-api")
            }
            Self::XfyunNativeWebSocketOfficialVendorApi => {
                cfg!(feature = "line-xfyun-native-websocket-official-vendor-api")
            }
            Self::FreeBuffWebReverseApi => cfg!(feature = "line-freebuff-web-reverse-api"),
            Self::ProducerWebReverseApi => cfg!(feature = "line-producer-web-reverse-api"),
            Self::GrokWebReverseApi => cfg!(feature = "line-grok-web-reverse-api"),
            Self::SunoWebReverseApi => cfg!(feature = "line-suno-web-reverse-api"),
            Self::UdioWebReverseApi => cfg!(feature = "line-udio-web-reverse-api"),
            Self::LumaLabsWebReverseApi => cfg!(feature = "line-lumalabs-web-reverse-api"),
            Self::PerplexitySearchOfficialVendorApi => {
                cfg!(feature = "line-perplexity-search-official-vendor-api")
            }
            Self::TavilySearchOfficialVendorApi => {
                cfg!(feature = "line-tavily-search-official-vendor-api")
            }
            Self::ExaSearchOfficialVendorApi => {
                cfg!(feature = "line-exa-search-official-vendor-api")
            }
            Self::JinaSearchOfficialVendorApi => {
                cfg!(feature = "line-jina-search-official-vendor-api")
            }
            Self::JinaReaderOfficialVendorApi => {
                cfg!(feature = "line-jina-reader-official-vendor-api")
            }
            Self::LinkupSearchOfficialVendorApi => {
                cfg!(feature = "line-linkup-search-official-vendor-api")
            }
            Self::YouSearchOfficialVendorApi => {
                cfg!(feature = "line-you-search-official-vendor-api")
            }
            Self::WebSearchApiSearchOfficialVendorApi => {
                cfg!(feature = "line-websearchapi-search-official-vendor-api")
            }
            Self::QwenOfficialApi => cfg!(feature = "line-qwen-official-api"),
            Self::QwenWebReverse => cfg!(feature = "line-qwen-web-reverse"),
            Self::ChataibotWebReverse => cfg!(feature = "line-chataibot-web-reverse"),
            Self::KiroOfficialVendorApi => cfg!(feature = "line-kiro-official-vendor-api"),
        }
    }
}

pub fn line_for_protocol_profile(profile: &str) -> Option<RefactoredImplementationLine> {
    match profile.trim() {
        "gemini_web_reverse_modular" | "gemini-web-chat-modular" => {
            Some(RefactoredImplementationLine::GeminiWebReverse)
        }
        "gemini_canvas_program_web_reverse_modular" | "gemini-canvas-program-relay" => {
            Some(RefactoredImplementationLine::GeminiCanvasProgram)
        }
        "aistudio_official_api"
        | "google_gemini_api"
        | "google_gemini_api_modular"
        | "aistudio-official-api"
        | "gemini-api"
        | "gemini-api-modular" => Some(RefactoredImplementationLine::AIStudioOfficial),
        "azure" | "azure_openai" | "azure-openai" => {
            Some(RefactoredImplementationLine::AzureOpenAIOfficialVendorApi)
        }
        "anthropic" | "anthropic_messages" | "anthropic-compatible" => {
            Some(RefactoredImplementationLine::AnthropicMessagesOfficialModelApi)
        }
        "aws_bedrock" | "aws-bedrock" | "bedrock" | "bedrock_converse" | "bedrock-converse" => {
            Some(RefactoredImplementationLine::AwsBedrockConverseOfficialModelApi)
        }
        "aistudio_web_reverse" | "aistudio-web-reverse" | "aistudio_web" | "aistudio" => {
            Some(RefactoredImplementationLine::AIStudioWebReverse)
        }
        "chatgpt_official_api" | "openai" | "openai_platform" | "openai-platform" => {
            Some(RefactoredImplementationLine::ChatGptOfficialApi)
        }
        "chatgpt_codex_oauth_official_api"
        | "chatgpt-codex-oauth-official-api"
        | "chatgpt_codex_backend"
        | "chatgpt-codex-backend"
        | "codex" => Some(RefactoredImplementationLine::ChatGptCodexOAuthOfficial),
        "chatgpt_web_reverse" | "chatgpt-web-reverse" | "chatgpt_web" | "chatgpt_web_chat" => {
            Some(RefactoredImplementationLine::ChatGptWebReverse)
        }
        "cohere" | "cohere_chat" | "cohere-chat" | "cohere_chat_v2" => {
            Some(RefactoredImplementationLine::CohereChatOfficialModelApi)
        }
        "groq" | "groq-openai" | "groq_openai" => {
            Some(RefactoredImplementationLine::GroqOpenAiOfficialVendorApi)
        }
        "nvidia" | "nvidia-openai" | "nvidia_openai" | "nvidia-nim" | "nvidia_nim" => {
            Some(RefactoredImplementationLine::NvidiaOpenAiOfficialVendorApi)
        }
        "together" | "together-openai" | "together_openai" => {
            Some(RefactoredImplementationLine::TogetherOpenAiAggregatorApi)
        }
        "openrouter" | "openrouter-openai" | "openrouter_openai" => {
            Some(RefactoredImplementationLine::OpenRouterOpenAiAggregatorApi)
        }
        "deepseek" | "deepseek-openai" | "deepseek_openai" => {
            Some(RefactoredImplementationLine::DeepSeekOpenAiOfficialModelApi)
        }
        "mistral" | "mistral-openai" | "mistral_openai" => {
            Some(RefactoredImplementationLine::MistralOpenAiOfficialModelApi)
        }
        "xai" | "xai-openai" | "xai_openai" => {
            Some(RefactoredImplementationLine::XaiOpenAiOfficialVendorApi)
        }
        "perplexity_chat" | "perplexity-chat" => {
            Some(RefactoredImplementationLine::PerplexityChatOfficialVendorApi)
        }
        "freebuff" | "freebuff-compatible" | "codebuff" => {
            Some(RefactoredImplementationLine::FreeBuffWebReverseApi)
        }
        "xfyun_openai" | "xfyun-openai" | "xfyun" => {
            Some(RefactoredImplementationLine::XfyunOpenAiOfficialVendorApi)
        }
        "xfyun_native_websocket" | "xfyun-native-websocket" | "xfyun_websocket" => {
            Some(RefactoredImplementationLine::XfyunNativeWebSocketOfficialVendorApi)
        }
        "producer" | "producer-images" | "producer_images" | "producer-music"
        | "producer_music" | "producer-videos" | "producer_videos" => {
            Some(RefactoredImplementationLine::ProducerWebReverseApi)
        }
        "grok_web" | "grok-web" | "grok" => Some(RefactoredImplementationLine::GrokWebReverseApi),
        "suno" | "suno-music" | "suno_music" | "suno-images" | "suno_images" | "suno-videos"
        | "suno_videos" => Some(RefactoredImplementationLine::SunoWebReverseApi),
        "udio" | "udio-music" | "udio_music" | "udio-images" | "udio_images" | "udio-videos"
        | "udio_videos" => Some(RefactoredImplementationLine::UdioWebReverseApi),
        "lumalabs" | "luma" | "luma-labs" | "lumalabs-images" | "lumalabs_images"
        | "lumalabs-videos" | "lumalabs_videos" | "lumalabs-audio" | "lumalabs_audio" => {
            Some(RefactoredImplementationLine::LumaLabsWebReverseApi)
        }
        "perplexity_search" | "perplexity-search" => {
            Some(RefactoredImplementationLine::PerplexitySearchOfficialVendorApi)
        }
        "tavily" | "tavily-search" | "tavily_search" => {
            Some(RefactoredImplementationLine::TavilySearchOfficialVendorApi)
        }
        "exa" | "exa-search" | "exa_search" => {
            Some(RefactoredImplementationLine::ExaSearchOfficialVendorApi)
        }
        "jina_search" | "jina-search" => {
            Some(RefactoredImplementationLine::JinaSearchOfficialVendorApi)
        }
        "jina_reader" | "jina-reader" => {
            Some(RefactoredImplementationLine::JinaReaderOfficialVendorApi)
        }
        "linkup" | "linkup-search" | "linkup_search" => {
            Some(RefactoredImplementationLine::LinkupSearchOfficialVendorApi)
        }
        "you_search" | "you-search" | "you" => {
            Some(RefactoredImplementationLine::YouSearchOfficialVendorApi)
        }
        "websearchapi" | "websearchapi-search" | "websearchapi_search" => {
            Some(RefactoredImplementationLine::WebSearchApiSearchOfficialVendorApi)
        }
        "qwen_dashscope_openai"
        | "qwen-dashscope-openai"
        | "qwen_coding_plan_openai"
        | "qwen-coding-plan-openai"
        | "qwen_coding_plan_anthropic"
        | "qwen-coding-plan-anthropic"
        | "qwen_official_api"
        | "qwen-official-api" => Some(RefactoredImplementationLine::QwenOfficialApi),
        "qwen_web_chat"
        | "qwen-web-chat"
        | "qwen_web"
        | "qwen-web"
        | "qwen-webui"
        | "qwen_webui_replay"
        | "qwen-webui-replay"
        | "qwen_webui_replay_live"
        | "qwen-webui-replay-live" => Some(RefactoredImplementationLine::QwenWebReverse),
        "chataibot" | "chataibot-images" | "chataibot_web_reverse" | "chataibot-web-reverse" => {
            Some(RefactoredImplementationLine::ChataibotWebReverse)
        }
        "kiro" | "kiro-compatible" | "kiro_official_vendor_api" | "kiro-official-vendor-api" => {
            Some(RefactoredImplementationLine::KiroOfficialVendorApi)
        }
        "google_agent_platform_official_api"
        | "google-agent-platform-official-api"
        | "google-agent-platform"
        | "vertex_official_api"
        | "vertex-official-api"
        | "google_vertex_gemini"
        | "vertex-gemini" => Some(RefactoredImplementationLine::GoogleAgentPlatformOfficial),
        _ => None,
    }
}

pub fn line_for_adapter(adapter: &str) -> Option<RefactoredImplementationLine> {
    match adapter.trim() {
        "gemini_web_compatible" | "gemini_web_reverse_modular_compatible" => {
            Some(RefactoredImplementationLine::GeminiWebReverse)
        }
        "gemini_canvas_program_web_reverse_compatible" => {
            Some(RefactoredImplementationLine::GeminiCanvasProgram)
        }
        "chatgpt_web_reverse_compatible" => Some(RefactoredImplementationLine::ChatGptWebReverse),
        "aistudio_web_reverse_compatible" => Some(RefactoredImplementationLine::AIStudioWebReverse),
        "qwen_web_compatible" => Some(RefactoredImplementationLine::QwenWebReverse),
        "chataibot_compatible" => Some(RefactoredImplementationLine::ChataibotWebReverse),
        "grok_compatible" => Some(RefactoredImplementationLine::GrokWebReverseApi),
        "suno_compatible" => Some(RefactoredImplementationLine::SunoWebReverseApi),
        "udio_compatible" => Some(RefactoredImplementationLine::UdioWebReverseApi),
        "lumalabs_compatible" => Some(RefactoredImplementationLine::LumaLabsWebReverseApi),
        "freebuff_compatible" => Some(RefactoredImplementationLine::FreeBuffWebReverseApi),
        "xfyun_websocket_compatible" => {
            Some(RefactoredImplementationLine::XfyunNativeWebSocketOfficialVendorApi)
        }
        "producer_compatible" => Some(RefactoredImplementationLine::ProducerWebReverseApi),
        "kiro_compatible" => Some(RefactoredImplementationLine::KiroOfficialVendorApi),
        "bedrock_converse_compatible" => {
            Some(RefactoredImplementationLine::AwsBedrockConverseOfficialModelApi)
        }
        "cohere_compatible" => Some(RefactoredImplementationLine::CohereChatOfficialModelApi),
        "search_api_compatible" | "linkup_compatible" => None,
        _ => None,
    }
}

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
mod tests {
    use super::*;
    use crate::routing::candidate::ProviderAccountPayload;
    use std::collections::HashMap;

    fn make_payload(adapter: &str, base_url: &str) -> ProviderAccountPayload {
        ProviderAccountPayload {
            adapter: adapter.to_string(),
            base_url: base_url.to_string(),
            api_key: "test".to_string(),
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
    fn payload_inference_distinguishes_aistudio_and_agent_platform_official() {
        assert_eq!(
            line_for_payload(&make_payload(
                "gemini_api_compatible",
                "https://generativelanguage.googleapis.com/v1beta"
            )),
            Some(RefactoredImplementationLine::AIStudioOfficial)
        );
        assert_eq!(
            line_for_payload(&make_payload(
                "gemini_api_compatible",
                "https://aiplatform.googleapis.com/v1/projects/demo/locations/us-central1/publishers/google"
            )),
            Some(RefactoredImplementationLine::GoogleAgentPlatformOfficial)
        );
    }

    #[test]
    fn payload_inference_distinguishes_chatgpt_official_codex_and_web_reverse() {
        assert_eq!(
            line_for_payload(&make_payload(
                "openai_compatible",
                "https://api.openai.com/v1"
            )),
            Some(RefactoredImplementationLine::ChatGptOfficialApi)
        );
        assert_eq!(
            line_for_payload(&make_payload(
                "openai_compatible",
                "https://chatgpt.com/backend-api/codex"
            )),
            Some(RefactoredImplementationLine::ChatGptCodexOAuthOfficial)
        );
        assert_eq!(
            line_for_payload(&make_payload(
                "chatgpt_web_reverse_compatible",
                "https://chatgpt.com/backend-api/conversation"
            )),
            Some(RefactoredImplementationLine::ChatGptWebReverse)
        );
    }

    #[test]
    fn payload_inference_distinguishes_qwen_official_and_web_reverse() {
        assert_eq!(
            line_for_payload(&make_payload(
                "openai_compatible",
                "https://dashscope.aliyuncs.com/compatible-mode/v1"
            )),
            Some(RefactoredImplementationLine::QwenOfficialApi)
        );
        assert_eq!(
            line_for_payload(&make_payload(
                "openai_compatible",
                "https://coding.dashscope.aliyuncs.com/v1"
            )),
            Some(RefactoredImplementationLine::QwenOfficialApi)
        );
        assert_eq!(
            line_for_payload(&make_payload(
                "anthropic_compatible",
                "https://coding.dashscope.aliyuncs.com/apps/anthropic"
            )),
            Some(RefactoredImplementationLine::QwenOfficialApi)
        );
        assert_eq!(
            line_for_payload(&make_payload("qwen_web_compatible", "https://chat.qwen.ai")),
            Some(RefactoredImplementationLine::QwenWebReverse)
        );
    }

    #[test]
    fn protocol_profile_inference_includes_wave4_openai_compatible_profiles() {
        assert_eq!(
            line_for_protocol_profile("groq"),
            Some(RefactoredImplementationLine::GroqOpenAiOfficialVendorApi)
        );
        assert_eq!(
            line_for_protocol_profile("together"),
            Some(RefactoredImplementationLine::TogetherOpenAiAggregatorApi)
        );
        assert_eq!(
            line_for_protocol_profile("openrouter"),
            Some(RefactoredImplementationLine::OpenRouterOpenAiAggregatorApi)
        );
        assert_eq!(
            line_for_protocol_profile("deepseek"),
            Some(RefactoredImplementationLine::DeepSeekOpenAiOfficialModelApi)
        );
        assert_eq!(
            line_for_protocol_profile("mistral"),
            Some(RefactoredImplementationLine::MistralOpenAiOfficialModelApi)
        );
    }

    #[test]
    fn payload_inference_distinguishes_wave4_openai_compatible_base_urls() {
        assert_eq!(
            line_for_payload(&make_payload(
                "openai_compatible",
                "https://api.groq.com/openai/v1"
            )),
            Some(RefactoredImplementationLine::GroqOpenAiOfficialVendorApi)
        );
        assert_eq!(
            line_for_payload(&make_payload(
                "openai_compatible",
                "https://api.together.xyz/v1"
            )),
            Some(RefactoredImplementationLine::TogetherOpenAiAggregatorApi)
        );
        assert_eq!(
            line_for_payload(&make_payload(
                "openai_compatible",
                "https://openrouter.ai/api/v1"
            )),
            Some(RefactoredImplementationLine::OpenRouterOpenAiAggregatorApi)
        );
        assert_eq!(
            line_for_payload(&make_payload(
                "openai_compatible",
                "https://api.deepseek.com/v1"
            )),
            Some(RefactoredImplementationLine::DeepSeekOpenAiOfficialModelApi)
        );
        assert_eq!(
            line_for_payload(&make_payload(
                "openai_compatible",
                "https://api.mistral.ai/v1"
            )),
            Some(RefactoredImplementationLine::MistralOpenAiOfficialModelApi)
        );
    }

    #[test]
    fn protocol_profile_mapping_includes_remaining_unfinished_platform_lines() {
        assert_eq!(
            line_for_protocol_profile("xai"),
            Some(RefactoredImplementationLine::XaiOpenAiOfficialVendorApi)
        );
        assert_eq!(
            line_for_protocol_profile("perplexity_chat"),
            Some(RefactoredImplementationLine::PerplexityChatOfficialVendorApi)
        );
        assert_eq!(
            line_for_protocol_profile("freebuff"),
            Some(RefactoredImplementationLine::FreeBuffWebReverseApi)
        );
        assert_eq!(
            line_for_protocol_profile("xfyun_openai"),
            Some(RefactoredImplementationLine::XfyunOpenAiOfficialVendorApi)
        );
        assert_eq!(
            line_for_protocol_profile("xfyun_native_websocket"),
            Some(RefactoredImplementationLine::XfyunNativeWebSocketOfficialVendorApi)
        );
        assert_eq!(
            line_for_protocol_profile("producer"),
            Some(RefactoredImplementationLine::ProducerWebReverseApi)
        );
    }

    #[test]
    fn payload_inference_distinguishes_remaining_unfinished_platform_lines() {
        assert_eq!(
            line_for_payload(&make_payload("openai_compatible", "https://api.x.ai/v1")),
            Some(RefactoredImplementationLine::XaiOpenAiOfficialVendorApi)
        );
        assert_eq!(
            line_for_payload(&make_payload(
                "openai_compatible",
                "https://api.perplexity.ai"
            )),
            Some(RefactoredImplementationLine::PerplexityChatOfficialVendorApi)
        );
        assert_eq!(
            line_for_payload(&make_payload(
                "freebuff_compatible",
                "https://www.codebuff.com"
            )),
            Some(RefactoredImplementationLine::FreeBuffWebReverseApi)
        );
        assert_eq!(
            line_for_payload(&make_payload(
                "openai_compatible",
                "https://spark-api-open.xf-yun.com/v1"
            )),
            Some(RefactoredImplementationLine::XfyunOpenAiOfficialVendorApi)
        );
        assert_eq!(
            line_for_payload(&make_payload(
                "xfyun_websocket_compatible",
                "wss://spark-api.xf-yun.com/v1.1/chat"
            )),
            Some(RefactoredImplementationLine::XfyunNativeWebSocketOfficialVendorApi)
        );
        assert_eq!(
            line_for_payload(&make_payload(
                "producer_compatible",
                "https://www.flowmusic.app"
            )),
            Some(RefactoredImplementationLine::ProducerWebReverseApi)
        );
    }

    #[test]
    fn payload_inference_distinguishes_search_family_base_urls() {
        assert_eq!(
            line_for_payload(&make_payload(
                "search_api_compatible",
                "https://api.perplexity.ai"
            )),
            Some(RefactoredImplementationLine::PerplexitySearchOfficialVendorApi)
        );
        assert_eq!(
            line_for_payload(&make_payload(
                "search_api_compatible",
                "https://api.tavily.com"
            )),
            Some(RefactoredImplementationLine::TavilySearchOfficialVendorApi)
        );
        assert_eq!(
            line_for_payload(&make_payload("search_api_compatible", "https://api.exa.ai")),
            Some(RefactoredImplementationLine::ExaSearchOfficialVendorApi)
        );
        assert_eq!(
            line_for_payload(&make_payload(
                "search_api_compatible",
                "https://s.jina.ai/search"
            )),
            Some(RefactoredImplementationLine::JinaSearchOfficialVendorApi)
        );
        assert_eq!(
            line_for_payload(&make_payload("search_api_compatible", "https://r.jina.ai")),
            Some(RefactoredImplementationLine::JinaReaderOfficialVendorApi)
        );
        assert_eq!(
            line_for_payload(&make_payload("linkup_compatible", "https://api.linkup.so")),
            Some(RefactoredImplementationLine::LinkupSearchOfficialVendorApi)
        );
        assert_eq!(
            line_for_payload(&make_payload(
                "search_api_compatible",
                "https://api.ydc-index.io"
            )),
            Some(RefactoredImplementationLine::YouSearchOfficialVendorApi)
        );
        assert_eq!(
            line_for_payload(&make_payload(
                "search_api_compatible",
                "https://api.websearchapi.com"
            )),
            Some(RefactoredImplementationLine::WebSearchApiSearchOfficialVendorApi)
        );
    }

    #[test]
    fn payload_inference_distinguishes_azure_anthropic_bedrock_and_cohere() {
        assert_eq!(
            line_for_payload(&make_payload(
                "openai_compatible",
                "https://example.openai.azure.com/openai/v1"
            )),
            Some(RefactoredImplementationLine::AzureOpenAIOfficialVendorApi)
        );
        assert_eq!(
            line_for_payload(&make_payload(
                "anthropic_compatible",
                "https://api.anthropic.com"
            )),
            Some(RefactoredImplementationLine::AnthropicMessagesOfficialModelApi)
        );
        assert_eq!(
            line_for_payload(&make_payload(
                "bedrock_converse_compatible",
                "https://bedrock-runtime.us-east-1.amazonaws.com"
            )),
            Some(RefactoredImplementationLine::AwsBedrockConverseOfficialModelApi)
        );
        assert_eq!(
            line_for_payload(&make_payload("cohere_compatible", "https://api.cohere.ai")),
            Some(RefactoredImplementationLine::CohereChatOfficialModelApi)
        );
    }

    #[test]
    fn protocol_profile_mapping_includes_wave3_official_api_lines() {
        assert_eq!(
            line_for_protocol_profile("azure_openai"),
            Some(RefactoredImplementationLine::AzureOpenAIOfficialVendorApi)
        );
        assert_eq!(
            line_for_protocol_profile("anthropic"),
            Some(RefactoredImplementationLine::AnthropicMessagesOfficialModelApi)
        );
        assert_eq!(
            line_for_protocol_profile("aws_bedrock"),
            Some(RefactoredImplementationLine::AwsBedrockConverseOfficialModelApi)
        );
        assert_eq!(
            line_for_protocol_profile("cohere"),
            Some(RefactoredImplementationLine::CohereChatOfficialModelApi)
        );
    }

    #[test]
    fn protocol_profile_mapping_includes_nvidia_and_grok_lines() {
        assert_eq!(
            line_for_protocol_profile("nvidia"),
            Some(RefactoredImplementationLine::NvidiaOpenAiOfficialVendorApi)
        );
        assert_eq!(
            line_for_protocol_profile("grok_web"),
            Some(RefactoredImplementationLine::GrokWebReverseApi)
        );
    }

    #[test]
    fn payload_inference_distinguishes_nvidia_and_grok_lines() {
        assert_eq!(
            line_for_payload(&make_payload(
                "openai_compatible",
                "https://integrate.api.nvidia.com/v1"
            )),
            Some(RefactoredImplementationLine::NvidiaOpenAiOfficialVendorApi)
        );
        assert_eq!(
            line_for_payload(&make_payload("grok_compatible", "https://grok.com")),
            Some(RefactoredImplementationLine::GrokWebReverseApi)
        );
    }

    #[test]
    fn protocol_profile_mapping_includes_search_family_lines() {
        assert_eq!(
            line_for_protocol_profile("perplexity_search"),
            Some(RefactoredImplementationLine::PerplexitySearchOfficialVendorApi)
        );
        assert_eq!(
            line_for_protocol_profile("tavily"),
            Some(RefactoredImplementationLine::TavilySearchOfficialVendorApi)
        );
        assert_eq!(
            line_for_protocol_profile("exa"),
            Some(RefactoredImplementationLine::ExaSearchOfficialVendorApi)
        );
        assert_eq!(
            line_for_protocol_profile("jina_search"),
            Some(RefactoredImplementationLine::JinaSearchOfficialVendorApi)
        );
        assert_eq!(
            line_for_protocol_profile("jina_reader"),
            Some(RefactoredImplementationLine::JinaReaderOfficialVendorApi)
        );
        assert_eq!(
            line_for_protocol_profile("linkup"),
            Some(RefactoredImplementationLine::LinkupSearchOfficialVendorApi)
        );
        assert_eq!(
            line_for_protocol_profile("you_search"),
            Some(RefactoredImplementationLine::YouSearchOfficialVendorApi)
        );
        assert_eq!(
            line_for_protocol_profile("websearchapi"),
            Some(RefactoredImplementationLine::WebSearchApiSearchOfficialVendorApi)
        );
    }

    macro_rules! search_protocol_profile_compile_tests {
        (
            feature = $feature:literal,
            profiles = [$($profile:literal),+ $(,)?],
            line = $line:literal,
            enabled = $enabled_test:ident,
            disabled = $disabled_test:ident
        ) => {
            #[cfg(feature = $feature)]
            #[test]
            fn $enabled_test() {
                for profile in [$($profile),+] {
                    assert!(is_protocol_profile_compiled_in(profile), "{profile}");
                    ensure_protocol_profile_compiled(profile)
                        .expect("search line feature enabled");
                }
            }

            #[cfg(not(feature = $feature))]
            #[test]
            fn $disabled_test() {
                for profile in [$($profile),+] {
                    assert!(!is_protocol_profile_compiled_in(profile), "{profile}");
                    let error = ensure_protocol_profile_compiled(profile)
                        .expect_err("search line feature disabled");
                    assert_eq!(
                        error.code.as_deref(),
                        Some("gateway_provider_line_compiled_out")
                    );
                    assert!(error.message.contains($line));
                    assert!(error.message.contains($feature));
                }
            }
        };
    }

    search_protocol_profile_compile_tests!(
        feature = "line-perplexity-search-official-vendor-api",
        profiles = ["perplexity_search", "perplexity-search"],
        line = "perplexity_search",
        enabled = perplexity_search_protocol_profiles_compile_when_feature_enabled,
        disabled = perplexity_search_protocol_profiles_fail_compile_check_when_feature_disabled
    );

    search_protocol_profile_compile_tests!(
        feature = "line-tavily-search-official-vendor-api",
        profiles = ["tavily", "tavily-search"],
        line = "tavily",
        enabled = tavily_search_protocol_profiles_compile_when_feature_enabled,
        disabled = tavily_search_protocol_profiles_fail_compile_check_when_feature_disabled
    );

    search_protocol_profile_compile_tests!(
        feature = "line-exa-search-official-vendor-api",
        profiles = ["exa", "exa-search"],
        line = "exa",
        enabled = exa_search_protocol_profiles_compile_when_feature_enabled,
        disabled = exa_search_protocol_profiles_fail_compile_check_when_feature_disabled
    );

    search_protocol_profile_compile_tests!(
        feature = "line-jina-search-official-vendor-api",
        profiles = ["jina_search", "jina-search"],
        line = "jina_search",
        enabled = jina_search_protocol_profiles_compile_when_feature_enabled,
        disabled = jina_search_protocol_profiles_fail_compile_check_when_feature_disabled
    );

    search_protocol_profile_compile_tests!(
        feature = "line-jina-reader-official-vendor-api",
        profiles = ["jina_reader", "jina-reader"],
        line = "jina_reader",
        enabled = jina_reader_protocol_profiles_compile_when_feature_enabled,
        disabled = jina_reader_protocol_profiles_fail_compile_check_when_feature_disabled
    );

    search_protocol_profile_compile_tests!(
        feature = "line-linkup-search-official-vendor-api",
        profiles = ["linkup", "linkup-search", "linkup_search"],
        line = "linkup",
        enabled = linkup_search_protocol_profiles_compile_when_feature_enabled,
        disabled = linkup_search_protocol_profiles_fail_compile_check_when_feature_disabled
    );

    search_protocol_profile_compile_tests!(
        feature = "line-you-search-official-vendor-api",
        profiles = ["you_search", "you-search", "you"],
        line = "you_search",
        enabled = you_search_protocol_profiles_compile_when_feature_enabled,
        disabled = you_search_protocol_profiles_fail_compile_check_when_feature_disabled
    );

    search_protocol_profile_compile_tests!(
        feature = "line-websearchapi-search-official-vendor-api",
        profiles = ["websearchapi", "websearchapi-search", "websearchapi_search"],
        line = "websearchapi",
        enabled = websearchapi_search_protocol_profiles_compile_when_feature_enabled,
        disabled = websearchapi_search_protocol_profiles_fail_compile_check_when_feature_disabled
    );

    search_protocol_profile_compile_tests!(
        feature = "line-xai-openai-official-vendor-api",
        profiles = ["xai", "xai-openai", "xai_openai"],
        line = "xai",
        enabled = xai_openai_protocol_profiles_compile_when_feature_enabled,
        disabled = xai_openai_protocol_profiles_fail_compile_check_when_feature_disabled
    );

    search_protocol_profile_compile_tests!(
        feature = "line-perplexity-chat-official-vendor-api",
        profiles = ["perplexity_chat", "perplexity-chat"],
        line = "perplexity_chat",
        enabled = perplexity_chat_protocol_profiles_compile_when_feature_enabled,
        disabled = perplexity_chat_protocol_profiles_fail_compile_check_when_feature_disabled
    );

    search_protocol_profile_compile_tests!(
        feature = "line-xfyun-openai-official-vendor-api",
        profiles = ["xfyun_openai", "xfyun-openai", "xfyun"],
        line = "xfyun_openai",
        enabled = xfyun_openai_protocol_profiles_compile_when_feature_enabled,
        disabled = xfyun_openai_protocol_profiles_fail_compile_check_when_feature_disabled
    );

    search_protocol_profile_compile_tests!(
        feature = "line-xfyun-native-websocket-official-vendor-api",
        profiles = [
            "xfyun_native_websocket",
            "xfyun-native-websocket",
            "xfyun_websocket"
        ],
        line = "xfyun_native_websocket",
        enabled = xfyun_native_websocket_protocol_profiles_compile_when_feature_enabled,
        disabled =
            xfyun_native_websocket_protocol_profiles_fail_compile_check_when_feature_disabled
    );

    search_protocol_profile_compile_tests!(
        feature = "line-freebuff-web-reverse-api",
        profiles = ["freebuff", "freebuff-compatible", "codebuff"],
        line = "freebuff",
        enabled = freebuff_web_reverse_protocol_profiles_compile_when_feature_enabled,
        disabled = freebuff_web_reverse_protocol_profiles_fail_compile_check_when_feature_disabled
    );

    search_protocol_profile_compile_tests!(
        feature = "line-producer-web-reverse-api",
        profiles = [
            "producer",
            "producer-images",
            "producer-music",
            "producer-videos"
        ],
        line = "producer",
        enabled = producer_web_reverse_protocol_profiles_compile_when_feature_enabled,
        disabled = producer_web_reverse_protocol_profiles_fail_compile_check_when_feature_disabled
    );

    #[test]
    fn protocol_profile_mapping_includes_media_platform_lines() {
        assert_eq!(
            line_for_protocol_profile("suno"),
            Some(RefactoredImplementationLine::SunoWebReverseApi)
        );
        assert_eq!(
            line_for_protocol_profile("suno-videos"),
            Some(RefactoredImplementationLine::SunoWebReverseApi)
        );
        assert_eq!(
            line_for_protocol_profile("udio"),
            Some(RefactoredImplementationLine::UdioWebReverseApi)
        );
        assert_eq!(
            line_for_protocol_profile("udio-music"),
            Some(RefactoredImplementationLine::UdioWebReverseApi)
        );
        assert_eq!(
            line_for_protocol_profile("lumalabs"),
            Some(RefactoredImplementationLine::LumaLabsWebReverseApi)
        );
        assert_eq!(
            line_for_protocol_profile("luma-labs"),
            Some(RefactoredImplementationLine::LumaLabsWebReverseApi)
        );
    }

    #[test]
    fn adapter_and_payload_mapping_include_media_platform_lines() {
        assert_eq!(
            line_for_adapter("suno_compatible"),
            Some(RefactoredImplementationLine::SunoWebReverseApi)
        );
        assert_eq!(
            line_for_adapter("udio_compatible"),
            Some(RefactoredImplementationLine::UdioWebReverseApi)
        );
        assert_eq!(
            line_for_adapter("lumalabs_compatible"),
            Some(RefactoredImplementationLine::LumaLabsWebReverseApi)
        );
        assert_eq!(
            line_for_payload(&make_payload(
                "suno_compatible",
                "https://studio-api-prod.suno.com"
            )),
            Some(RefactoredImplementationLine::SunoWebReverseApi)
        );
        assert_eq!(
            line_for_payload(&make_payload("udio_compatible", "https://www.udio.com")),
            Some(RefactoredImplementationLine::UdioWebReverseApi)
        );
        assert_eq!(
            line_for_payload(&make_payload(
                "lumalabs_compatible",
                "https://app.lumalabs.ai"
            )),
            Some(RefactoredImplementationLine::LumaLabsWebReverseApi)
        );
    }

    #[test]
    fn media_web_reverse_protocol_profiles_compile_when_feature_enabled() {
        let cases = [
            (
                ["suno", "suno-music", "suno-videos"].as_slice(),
                cfg!(feature = "line-suno-web-reverse-api"),
            ),
            (
                ["udio", "udio-music", "udio-videos"].as_slice(),
                cfg!(feature = "line-udio-web-reverse-api"),
            ),
            (
                ["lumalabs", "luma-labs", "lumalabs-videos"].as_slice(),
                cfg!(feature = "line-lumalabs-web-reverse-api"),
            ),
        ];
        for (profiles, enabled) in cases {
            if !enabled {
                continue;
            }
            for profile in profiles {
                assert!(is_protocol_profile_compiled_in(profile), "{profile}");
                ensure_protocol_profile_compiled(profile).expect("media platform line enabled");
            }
        }
    }

    #[test]
    fn media_web_reverse_protocol_profiles_fail_compile_check_when_feature_disabled() {
        let cases = [
            (
                ["suno", "suno-music", "suno-videos"].as_slice(),
                "suno",
                "line-suno-web-reverse-api",
                cfg!(feature = "line-suno-web-reverse-api"),
            ),
            (
                ["udio", "udio-music", "udio-videos"].as_slice(),
                "udio",
                "line-udio-web-reverse-api",
                cfg!(feature = "line-udio-web-reverse-api"),
            ),
            (
                ["lumalabs", "luma-labs", "lumalabs-videos"].as_slice(),
                "lumalabs",
                "line-lumalabs-web-reverse-api",
                cfg!(feature = "line-lumalabs-web-reverse-api"),
            ),
        ];
        for (profiles, line, feature, enabled) in cases {
            if enabled {
                continue;
            }
            for profile in profiles {
                assert!(!is_protocol_profile_compiled_in(profile), "{profile}");
                let error = ensure_protocol_profile_compiled(profile)
                    .expect_err("media platform line disabled");
                assert_eq!(
                    error.code.as_deref(),
                    Some("gateway_provider_line_compiled_out")
                );
                assert!(error.message.contains(line));
                assert!(error.message.contains(feature));
            }
        }
    }
    #[test]
    fn compiled_out_error_uses_stable_code() {
        let error = compiled_out_error_for_line(RefactoredImplementationLine::AIStudioWebReverse);
        assert_eq!(
            error.code.as_deref(),
            Some("gateway_provider_line_compiled_out")
        );
        assert_eq!(error.http_status, Some(409));
    }

    #[cfg(not(feature = "line-qwen-official-api"))]
    #[test]
    fn qwen_official_payload_fails_compiled_check_when_feature_disabled() {
        let payload = make_payload(
            "openai_compatible",
            "https://dashscope.aliyuncs.com/compatible-mode/v1",
        );
        let error = ensure_payload_compiled(&payload).expect_err("qwen official line disabled");
        assert_eq!(
            error.code.as_deref(),
            Some("gateway_provider_line_compiled_out")
        );
        assert!(error.message.contains("qwen_official_api"));
        assert!(error.message.contains("line-qwen-official-api"));
    }

    #[cfg(feature = "line-qwen-official-api")]
    #[test]
    fn qwen_official_protocol_profiles_compile_when_feature_enabled() {
        for profile in [
            "qwen_dashscope_openai",
            "qwen_coding_plan_openai",
            "qwen_coding_plan_anthropic",
            "qwen_official_api",
        ] {
            assert!(is_protocol_profile_compiled_in(profile), "{profile}");
            ensure_protocol_profile_compiled(profile).expect("qwen official line enabled");
        }
    }

    #[cfg(not(feature = "line-qwen-official-api"))]
    #[test]
    fn qwen_official_protocol_profiles_fail_compile_check_when_feature_disabled() {
        for profile in [
            "qwen_dashscope_openai",
            "qwen_coding_plan_openai",
            "qwen_coding_plan_anthropic",
            "qwen_official_api",
        ] {
            assert!(!is_protocol_profile_compiled_in(profile), "{profile}");
            let error =
                ensure_protocol_profile_compiled(profile).expect_err("qwen official line disabled");
            assert_eq!(
                error.code.as_deref(),
                Some("gateway_provider_line_compiled_out")
            );
            assert!(error.message.contains("qwen_official_api"));
            assert!(error.message.contains("line-qwen-official-api"));
        }
    }

    #[cfg(not(feature = "line-qwen-web-reverse"))]
    #[test]
    fn qwen_web_payload_fails_compiled_check_when_feature_disabled() {
        let payload = make_payload("qwen_web_compatible", "https://chat.qwen.ai");
        let error = ensure_payload_compiled(&payload).expect_err("qwen web line disabled");
        assert_eq!(
            error.code.as_deref(),
            Some("gateway_provider_line_compiled_out")
        );
        assert!(error.message.contains("qwen_web_reverse"));
        assert!(error.message.contains("line-qwen-web-reverse"));
    }

    #[cfg(feature = "line-qwen-web-reverse")]
    #[test]
    fn qwen_web_protocol_profiles_compile_when_feature_enabled() {
        for profile in [
            "qwen_web_chat",
            "qwen_web",
            "qwen-web",
            "qwen-webui",
            "qwen-webui-replay",
            "qwen-webui-replay-live",
        ] {
            assert!(is_protocol_profile_compiled_in(profile), "{profile}");
            ensure_protocol_profile_compiled(profile).expect("qwen web line enabled");
        }
    }

    #[cfg(not(feature = "line-qwen-web-reverse"))]
    #[test]
    fn qwen_web_protocol_profiles_fail_compile_check_when_feature_disabled() {
        for profile in [
            "qwen_web_chat",
            "qwen_web",
            "qwen-web",
            "qwen-webui",
            "qwen-webui-replay",
            "qwen-webui-replay-live",
        ] {
            assert!(!is_protocol_profile_compiled_in(profile), "{profile}");
            let error =
                ensure_protocol_profile_compiled(profile).expect_err("qwen web line disabled");
            assert_eq!(
                error.code.as_deref(),
                Some("gateway_provider_line_compiled_out")
            );
            assert!(error.message.contains("qwen_web_reverse"));
            assert!(error.message.contains("line-qwen-web-reverse"));
        }
    }

    #[cfg(feature = "line-nvidia-openai-official-vendor-api")]
    #[test]
    fn nvidia_protocol_profiles_compile_when_feature_enabled() {
        for profile in ["nvidia", "nvidia-openai", "nvidia_nim"] {
            assert!(is_protocol_profile_compiled_in(profile), "{profile}");
            ensure_protocol_profile_compiled(profile).expect("nvidia line enabled");
        }
    }

    #[cfg(not(feature = "line-nvidia-openai-official-vendor-api"))]
    #[test]
    fn nvidia_payload_fails_compiled_check_when_feature_disabled() {
        let payload = make_payload("openai_compatible", "https://integrate.api.nvidia.com/v1");
        let error = ensure_payload_compiled(&payload).expect_err("nvidia line disabled");
        assert_eq!(
            error.code.as_deref(),
            Some("gateway_provider_line_compiled_out")
        );
        assert!(error.message.contains("nvidia"));
        assert!(error
            .message
            .contains("line-nvidia-openai-official-vendor-api"));
    }

    #[cfg(not(feature = "line-nvidia-openai-official-vendor-api"))]
    #[test]
    fn nvidia_protocol_profiles_fail_compile_check_when_feature_disabled() {
        for profile in ["nvidia", "nvidia-openai", "nvidia_nim"] {
            assert!(!is_protocol_profile_compiled_in(profile), "{profile}");
            let error =
                ensure_protocol_profile_compiled(profile).expect_err("nvidia line disabled");
            assert_eq!(
                error.code.as_deref(),
                Some("gateway_provider_line_compiled_out")
            );
            assert!(error.message.contains("nvidia"));
            assert!(error
                .message
                .contains("line-nvidia-openai-official-vendor-api"));
        }
    }

    #[cfg(feature = "line-grok-web-reverse-api")]
    #[test]
    fn grok_protocol_profiles_compile_when_feature_enabled() {
        for profile in ["grok_web", "grok", "grok-web"] {
            assert!(is_protocol_profile_compiled_in(profile), "{profile}");
            ensure_protocol_profile_compiled(profile).expect("grok line enabled");
        }
    }

    #[cfg(not(feature = "line-grok-web-reverse-api"))]
    #[test]
    fn grok_payload_fails_compiled_check_when_feature_disabled() {
        let payload = make_payload("grok_compatible", "https://grok.com");
        let error = ensure_payload_compiled(&payload).expect_err("grok line disabled");
        assert_eq!(
            error.code.as_deref(),
            Some("gateway_provider_line_compiled_out")
        );
        assert!(error.message.contains("grok_web"));
        assert!(error.message.contains("line-grok-web-reverse-api"));
    }

    #[cfg(not(feature = "line-grok-web-reverse-api"))]
    #[test]
    fn grok_protocol_profiles_fail_compile_check_when_feature_disabled() {
        for profile in ["grok_web", "grok", "grok-web"] {
            assert!(!is_protocol_profile_compiled_in(profile), "{profile}");
            let error = ensure_protocol_profile_compiled(profile).expect_err("grok line disabled");
            assert_eq!(
                error.code.as_deref(),
                Some("gateway_provider_line_compiled_out")
            );
            assert!(error.message.contains("grok_web"));
            assert!(error.message.contains("line-grok-web-reverse-api"));
        }
    }
}
