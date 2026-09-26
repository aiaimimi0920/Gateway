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
pub const LINE_MUYUAN_OPENAI_AGGREGATOR_API_FEATURE: &str = "line-muyuan-openai-aggregator-api";
pub const LINE_POE_OPENAI_AGGREGATOR_API_FEATURE: &str = "line-poe-openai-aggregator-api";
pub const LINE_LONGCAT_OPENAI_OFFICIAL_MODEL_API_FEATURE: &str =
    "line-longcat-openai-official-model-api";
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
    MuyuanOpenAiAggregatorApi,
    PoeOpenAiAggregatorApi,
    LongCatOpenAiOfficialModelApi,
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
            Self::MuyuanOpenAiAggregatorApi => "muyuan",
            Self::PoeOpenAiAggregatorApi => "poe",
            Self::LongCatOpenAiOfficialModelApi => "longcat",
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
            Self::MuyuanOpenAiAggregatorApi => LINE_MUYUAN_OPENAI_AGGREGATOR_API_FEATURE,
            Self::PoeOpenAiAggregatorApi => LINE_POE_OPENAI_AGGREGATOR_API_FEATURE,
            Self::LongCatOpenAiOfficialModelApi => LINE_LONGCAT_OPENAI_OFFICIAL_MODEL_API_FEATURE,
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
            Self::MuyuanOpenAiAggregatorApi => {
                cfg!(feature = "line-muyuan-openai-aggregator-api")
            }
            Self::PoeOpenAiAggregatorApi => {
                cfg!(feature = "line-poe-openai-aggregator-api")
            }
            Self::LongCatOpenAiOfficialModelApi => {
                cfg!(feature = "line-longcat-openai-official-model-api")
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
