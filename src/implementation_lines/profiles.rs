use super::catalog::RefactoredImplementationLine;

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
        "muyuan" | "muyuan-openai" | "muyuan_openai" => {
            Some(RefactoredImplementationLine::MuyuanOpenAiAggregatorApi)
        }
        "poe" | "poe-openai" | "poe_openai" => {
            Some(RefactoredImplementationLine::PoeOpenAiAggregatorApi)
        }
        "longcat" | "longcat-openai" | "longcat_openai" => {
            Some(RefactoredImplementationLine::LongCatOpenAiOfficialModelApi)
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
        "gemini_web_compatible"
        | "gemini_web_reverse_modular_compatible"
        | "gemini_canvas_compatible"
        | "gemini_canvas_web_reverse_compatible" => {
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
