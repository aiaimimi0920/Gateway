pub const OPENAI_CHAT_FAMILY: &str = "openai_chat";
pub const OPENAI_LEGACY_COMPLETIONS_FAMILY: &str = "openai_legacy_completions";
pub const OPENAI_RESPONSES_FAMILY: &str = "openai_responses";
pub const OPENAI_REALTIME_FAMILY: &str = "openai_realtime";
pub const OPENAI_EMBEDDINGS_FAMILY: &str = "openai_embeddings";
pub const OPENAI_AUDIO_TRANSCRIPTIONS_FAMILY: &str = "openai_audio_transcriptions";
pub const OPENAI_AUDIO_SPEECH_FAMILY: &str = "openai_audio_speech";
pub const OPENAI_IMAGES_GENERATIONS_FAMILY: &str = "openai_images_generations";
pub const OPENAI_IMAGES_EDITS_FAMILY: &str = "openai_images_edits";
pub const OPENAI_MUSIC_GENERATIONS_FAMILY: &str = "openai_music_generations";
pub const OPENAI_VIDEOS_GENERATIONS_FAMILY: &str = "openai_videos_generations";
pub const ANTHROPIC_MESSAGES_FAMILY: &str = "anthropic_messages";
pub const GEMINI_GENERATE_CONTENT_FAMILY: &str = "gemini_generate_content";
pub const GEMINI_LIVE_FAMILY: &str = "gemini_live";
pub const BEDROCK_CONVERSE_FAMILY: &str = "bedrock_converse";
pub const COHERE_CHAT_FAMILY: &str = "cohere_chat";
pub const COHERE_CHAT_V2_FAMILY_ALIAS: &str = "cohere_chat_v2";
pub const CHATGPT_WEB_CHAT_FAMILY: &str = "chatgpt_web_chat";
pub const GEMINI_WEB_CHAT_FAMILY: &str = "gemini_web_chat";
pub const GEMINI_CANVAS_WEB_RELAY_FAMILY: &str = "gemini_canvas_web_relay";
pub const QWEN_WEB_CHAT_FAMILY: &str = "qwen_web_chat";
pub const SEARCH_API_FAMILY: &str = "search";
pub const PERPLEXITY_SEARCH_FAMILY: &str = "perplexity_search";
pub const TAVILY_SEARCH_FAMILY: &str = "tavily_search";
pub const EXA_SEARCH_FAMILY: &str = "exa_search";
pub const JINA_SEARCH_FAMILY: &str = "jina_search";
pub const JINA_READER_FAMILY: &str = "jina_reader";
pub const LINKUP_SEARCH_FAMILY: &str = "linkup_search";
pub const YOU_SEARCH_FAMILY: &str = "you_search";
pub const WEBSEARCHAPI_SEARCH_FAMILY: &str = "websearchapi_search";
pub const GEMINI_BUSINESS_IMAGES_FAMILY: &str = "gemini_business_images";
pub const CHATAIBOT_IMAGES_FAMILY: &str = "chataibot_images";
pub const LUMALABS_IMAGES_FAMILY: &str = "lumalabs_images";
pub const LUMALABS_AUDIO_FAMILY: &str = "lumalabs_audio";
pub const LUMALABS_VIDEOS_FAMILY: &str = "lumalabs_videos";
pub const GEMINI_CANVAS_IMAGES_FAMILY: &str = "gemini_canvas_images";
pub const GEMINI_CANVAS_MUSIC_FAMILY: &str = "gemini_canvas_music";
pub const GEMINI_CANVAS_VIDEOS_FAMILY: &str = "gemini_canvas_videos";
pub const PRODUCER_IMAGES_FAMILY: &str = "producer_images";
pub const PRODUCER_MUSIC_FAMILY: &str = "producer_music";
pub const PRODUCER_VIDEOS_FAMILY: &str = "producer_videos";
pub const SUNO_IMAGES_FAMILY: &str = "suno_images";
pub const SUNO_MUSIC_FAMILY: &str = "suno_music";
pub const SUNO_VIDEOS_FAMILY: &str = "suno_videos";
pub const UDIO_IMAGES_FAMILY: &str = "udio_images";
pub const UDIO_MUSIC_FAMILY: &str = "udio_music";
pub const UDIO_VIDEOS_FAMILY: &str = "udio_videos";

pub fn canonicalize_protocol_family_key(value: &str) -> String {
    let trimmed = value.trim().to_lowercase();
    match trimmed.as_str() {
        COHERE_CHAT_V2_FAMILY_ALIAS => COHERE_CHAT_FAMILY.to_string(),
        "search_api" => SEARCH_API_FAMILY.to_string(),
        "linkup" => LINKUP_SEARCH_FAMILY.to_string(),
        "perplexity-search" => PERPLEXITY_SEARCH_FAMILY.to_string(),
        "tavily" => TAVILY_SEARCH_FAMILY.to_string(),
        "exa" => EXA_SEARCH_FAMILY.to_string(),
        "you" => YOU_SEARCH_FAMILY.to_string(),
        "websearchapi" => WEBSEARCHAPI_SEARCH_FAMILY.to_string(),
        "jina-search" => JINA_SEARCH_FAMILY.to_string(),
        "jina-reader" => JINA_READER_FAMILY.to_string(),
        "openai_embeddings" | "embeddings" => OPENAI_EMBEDDINGS_FAMILY.to_string(),
        "openai_audio_transcriptions" | "audio_transcriptions" | "transcriptions" => {
            OPENAI_AUDIO_TRANSCRIPTIONS_FAMILY.to_string()
        }
        "openai_audio_speech" | "audio_speech" => OPENAI_AUDIO_SPEECH_FAMILY.to_string(),
        "openai_images_generations" | "images_generations" | "image_generations" => {
            OPENAI_IMAGES_GENERATIONS_FAMILY.to_string()
        }
        "openai_images_edits" | "images_edits" | "image_edits" => {
            OPENAI_IMAGES_EDITS_FAMILY.to_string()
        }
        "openai_music_generations" | "music_generations" => {
            OPENAI_MUSIC_GENERATIONS_FAMILY.to_string()
        }
        "openai_videos_generations" | "videos_generations" => {
            OPENAI_VIDEOS_GENERATIONS_FAMILY.to_string()
        }
        _ => trimmed,
    }
}

pub fn canonicalize_wire_protocol_family_key(value: &str) -> String {
    let trimmed = canonicalize_protocol_family_key(value);
    match trimmed.as_str() {
        "openai" | "openai_chat" | "openai_chat_completions" | "chat_completions" | "chat" => {
            OPENAI_CHAT_FAMILY.to_string()
        }
        "openai_legacy"
        | "openai_completions"
        | "openai_legacy_completions"
        | "legacy_completions"
        | "completions" => OPENAI_LEGACY_COMPLETIONS_FAMILY.to_string(),
        "openai_responses" | "responses" => OPENAI_RESPONSES_FAMILY.to_string(),
        "openai_realtime" | "realtime" => OPENAI_REALTIME_FAMILY.to_string(),
        OPENAI_EMBEDDINGS_FAMILY => OPENAI_EMBEDDINGS_FAMILY.to_string(),
        OPENAI_AUDIO_TRANSCRIPTIONS_FAMILY => OPENAI_AUDIO_TRANSCRIPTIONS_FAMILY.to_string(),
        OPENAI_AUDIO_SPEECH_FAMILY => OPENAI_AUDIO_SPEECH_FAMILY.to_string(),
        OPENAI_IMAGES_GENERATIONS_FAMILY => OPENAI_IMAGES_GENERATIONS_FAMILY.to_string(),
        OPENAI_IMAGES_EDITS_FAMILY => OPENAI_IMAGES_EDITS_FAMILY.to_string(),
        OPENAI_MUSIC_GENERATIONS_FAMILY => OPENAI_MUSIC_GENERATIONS_FAMILY.to_string(),
        OPENAI_VIDEOS_GENERATIONS_FAMILY => OPENAI_VIDEOS_GENERATIONS_FAMILY.to_string(),
        "anthropic" | "anthropic_messages" | "messages" => ANTHROPIC_MESSAGES_FAMILY.to_string(),
        "gemini" | "gemini_generate_content" | "generate_content" => {
            GEMINI_GENERATE_CONTENT_FAMILY.to_string()
        }
        "gemini_live" => GEMINI_LIVE_FAMILY.to_string(),
        "bedrock" | "bedrock_converse" | "converse" => BEDROCK_CONVERSE_FAMILY.to_string(),
        "cohere" | COHERE_CHAT_V2_FAMILY_ALIAS | COHERE_CHAT_FAMILY => {
            COHERE_CHAT_FAMILY.to_string()
        }
        "chatgpt_web" | "chatgpt_web_chat" | "chatgpt_web_reverse" => {
            CHATGPT_WEB_CHAT_FAMILY.to_string()
        }
        "aistudio_official_api" | "google_agent_platform_official_api" => {
            GEMINI_GENERATE_CONTENT_FAMILY.to_string()
        }
        "aistudio" | "aistudio_web" | "aistudio_web_reverse" => {
            GEMINI_GENERATE_CONTENT_FAMILY.to_string()
        }
        "gemini_web" | "gemini_web_chat" => GEMINI_WEB_CHAT_FAMILY.to_string(),
        "gemini_canvas_web_relay" | "gemini_canvas_browser_relay" => {
            GEMINI_CANVAS_WEB_RELAY_FAMILY.to_string()
        }
        "qwen_web" | "qwen_web_chat" => QWEN_WEB_CHAT_FAMILY.to_string(),
        "search" | "search_api" => SEARCH_API_FAMILY.to_string(),
        PERPLEXITY_SEARCH_FAMILY => PERPLEXITY_SEARCH_FAMILY.to_string(),
        TAVILY_SEARCH_FAMILY => TAVILY_SEARCH_FAMILY.to_string(),
        EXA_SEARCH_FAMILY => EXA_SEARCH_FAMILY.to_string(),
        JINA_SEARCH_FAMILY => JINA_SEARCH_FAMILY.to_string(),
        JINA_READER_FAMILY => JINA_READER_FAMILY.to_string(),
        LINKUP_SEARCH_FAMILY => LINKUP_SEARCH_FAMILY.to_string(),
        YOU_SEARCH_FAMILY => YOU_SEARCH_FAMILY.to_string(),
        WEBSEARCHAPI_SEARCH_FAMILY => WEBSEARCHAPI_SEARCH_FAMILY.to_string(),
        GEMINI_BUSINESS_IMAGES_FAMILY => GEMINI_BUSINESS_IMAGES_FAMILY.to_string(),
        CHATAIBOT_IMAGES_FAMILY => CHATAIBOT_IMAGES_FAMILY.to_string(),
        LUMALABS_IMAGES_FAMILY => LUMALABS_IMAGES_FAMILY.to_string(),
        LUMALABS_AUDIO_FAMILY => LUMALABS_AUDIO_FAMILY.to_string(),
        LUMALABS_VIDEOS_FAMILY => LUMALABS_VIDEOS_FAMILY.to_string(),
        GEMINI_CANVAS_IMAGES_FAMILY => GEMINI_CANVAS_IMAGES_FAMILY.to_string(),
        GEMINI_CANVAS_MUSIC_FAMILY => GEMINI_CANVAS_MUSIC_FAMILY.to_string(),
        GEMINI_CANVAS_VIDEOS_FAMILY => GEMINI_CANVAS_VIDEOS_FAMILY.to_string(),
        PRODUCER_IMAGES_FAMILY => PRODUCER_IMAGES_FAMILY.to_string(),
        PRODUCER_MUSIC_FAMILY => PRODUCER_MUSIC_FAMILY.to_string(),
        PRODUCER_VIDEOS_FAMILY => PRODUCER_VIDEOS_FAMILY.to_string(),
        SUNO_IMAGES_FAMILY => SUNO_IMAGES_FAMILY.to_string(),
        SUNO_MUSIC_FAMILY => SUNO_MUSIC_FAMILY.to_string(),
        SUNO_VIDEOS_FAMILY => SUNO_VIDEOS_FAMILY.to_string(),
        UDIO_IMAGES_FAMILY => UDIO_IMAGES_FAMILY.to_string(),
        UDIO_MUSIC_FAMILY => UDIO_MUSIC_FAMILY.to_string(),
        UDIO_VIDEOS_FAMILY => UDIO_VIDEOS_FAMILY.to_string(),
        _ => trimmed,
    }
}

pub fn is_openai_protocol_family(value: &str) -> bool {
    matches!(
        canonicalize_wire_protocol_family_key(value).as_str(),
        OPENAI_CHAT_FAMILY
            | OPENAI_LEGACY_COMPLETIONS_FAMILY
            | OPENAI_RESPONSES_FAMILY
            | OPENAI_REALTIME_FAMILY
            | OPENAI_EMBEDDINGS_FAMILY
            | OPENAI_AUDIO_TRANSCRIPTIONS_FAMILY
            | OPENAI_AUDIO_SPEECH_FAMILY
            | OPENAI_IMAGES_GENERATIONS_FAMILY
            | OPENAI_IMAGES_EDITS_FAMILY
            | OPENAI_MUSIC_GENERATIONS_FAMILY
            | OPENAI_VIDEOS_GENERATIONS_FAMILY
    ) || canonicalize_protocol_family_key(value) == "openai"
}

pub fn is_search_protocol_family(value: &str) -> bool {
    matches!(
        canonicalize_protocol_family_key(value).as_str(),
        SEARCH_API_FAMILY
            | PERPLEXITY_SEARCH_FAMILY
            | TAVILY_SEARCH_FAMILY
            | EXA_SEARCH_FAMILY
            | JINA_SEARCH_FAMILY
            | JINA_READER_FAMILY
            | LINKUP_SEARCH_FAMILY
            | YOU_SEARCH_FAMILY
            | WEBSEARCHAPI_SEARCH_FAMILY
    )
}

pub fn is_gemini_canvas_protocol_family(value: &str) -> bool {
    matches!(
        canonicalize_wire_protocol_family_key(value).as_str(),
        GEMINI_CANVAS_IMAGES_FAMILY
            | GEMINI_CANVAS_MUSIC_FAMILY
            | GEMINI_CANVAS_VIDEOS_FAMILY
            | GEMINI_GENERATE_CONTENT_FAMILY
    ) || canonicalize_protocol_family_key(value) == "gemini_canvas"
}

pub fn is_gemini_web_protocol_family(value: &str) -> bool {
    canonicalize_wire_protocol_family_key(value) == GEMINI_WEB_CHAT_FAMILY
        || canonicalize_protocol_family_key(value) == "gemini_web"
}

pub fn is_chatgpt_web_protocol_family(value: &str) -> bool {
    canonicalize_wire_protocol_family_key(value) == CHATGPT_WEB_CHAT_FAMILY
        || matches!(
            canonicalize_protocol_family_key(value).as_str(),
            "chatgpt_web" | "chatgpt_web_reverse"
        )
}

pub fn is_lumalabs_protocol_family(value: &str) -> bool {
    matches!(
        canonicalize_wire_protocol_family_key(value).as_str(),
        LUMALABS_IMAGES_FAMILY | LUMALABS_AUDIO_FAMILY | LUMALABS_VIDEOS_FAMILY
    ) || canonicalize_protocol_family_key(value) == "lumalabs"
}

pub fn is_producer_protocol_family(value: &str) -> bool {
    matches!(
        canonicalize_wire_protocol_family_key(value).as_str(),
        PRODUCER_IMAGES_FAMILY | PRODUCER_MUSIC_FAMILY | PRODUCER_VIDEOS_FAMILY
    ) || canonicalize_protocol_family_key(value) == "producer"
}

pub fn is_suno_protocol_family(value: &str) -> bool {
    matches!(
        canonicalize_wire_protocol_family_key(value).as_str(),
        SUNO_IMAGES_FAMILY | SUNO_MUSIC_FAMILY | SUNO_VIDEOS_FAMILY
    ) || canonicalize_protocol_family_key(value) == "suno"
}

pub fn is_udio_protocol_family(value: &str) -> bool {
    matches!(
        canonicalize_wire_protocol_family_key(value).as_str(),
        UDIO_IMAGES_FAMILY | UDIO_MUSIC_FAMILY | UDIO_VIDEOS_FAMILY
    ) || canonicalize_protocol_family_key(value) == "udio"
}

pub fn protocol_family_selector_matches(selector: &str, actual: &str) -> bool {
    let selector_key = canonicalize_protocol_family_key(selector);
    let actual_key = canonicalize_protocol_family_key(actual);
    let actual_wire = canonicalize_wire_protocol_family_key(&actual_key);

    match selector_key.as_str() {
        "openai" => is_openai_protocol_family(&actual_key),
        "anthropic" => {
            actual_key == "anthropic" || actual_wire.as_str() == ANTHROPIC_MESSAGES_FAMILY
        }
        "gemini" => {
            matches!(
                actual_wire.as_str(),
                GEMINI_GENERATE_CONTENT_FAMILY | GEMINI_LIVE_FAMILY
            ) || actual_key == "gemini"
        }
        "chatgpt_web" | "chatgpt_web_reverse" => is_chatgpt_web_protocol_family(&actual_key),
        "aistudio_official_api" | "google_agent_platform_official_api" => {
            matches!(
                actual_wire.as_str(),
                GEMINI_GENERATE_CONTENT_FAMILY | GEMINI_LIVE_FAMILY
            ) || matches!(
                actual_key.as_str(),
                "aistudio_official_api" | "google_agent_platform_official_api"
            )
        }
        "aistudio" | "aistudio_web" | "aistudio_web_reverse" => {
            actual_wire.as_str() == GEMINI_GENERATE_CONTENT_FAMILY
                || actual_key == "aistudio_web_reverse"
        }
        "gemini_web" => is_gemini_web_protocol_family(&actual_key),
        "bedrock" => actual_key == "bedrock" || actual_wire.as_str() == BEDROCK_CONVERSE_FAMILY,
        "cohere" => actual_key == "cohere" || actual_wire.as_str() == COHERE_CHAT_FAMILY,
        "search" => is_search_protocol_family(&actual_key),
        "gemini_business" => {
            actual_key == "gemini_business" || actual_wire.as_str() == GEMINI_BUSINESS_IMAGES_FAMILY
        }
        "chataibot" => actual_key == "chataibot" || actual_wire.as_str() == CHATAIBOT_IMAGES_FAMILY,
        "lumalabs" => is_lumalabs_protocol_family(&actual_key),
        "gemini_canvas" => is_gemini_canvas_protocol_family(&actual_key),
        "producer" => is_producer_protocol_family(&actual_key),
        "suno" => is_suno_protocol_family(&actual_key),
        "udio" => is_udio_protocol_family(&actual_key),
        _ => {
            selector_key == actual_key
                || canonicalize_wire_protocol_family_key(&selector_key) == actual_wire
        }
    }
}

pub fn canonicalize_protocol_profile_key(value: &str) -> String {
    let normalized = value
        .trim()
        .to_lowercase()
        .replace('-', "_")
        .replace(' ', "_");
    match normalized.as_str() {
        "openai" | "openai_platform" | "chatgpt_official_api" => "chatgpt_official_api".to_string(),
        "codex" | "chatgpt_codex_backend" | "chatgpt_codex_oauth_official_api" => {
            "chatgpt_codex_oauth_official_api".to_string()
        }
        "qwen" => "qwen_dashscope_openai".to_string(),
        "qwen_web" | "qwen_webui" | "qwen_webui_replay" | "qwen_webui_replay_live" => {
            "qwen_web_chat".to_string()
        }
        "chatgpt_web_chat" => "chatgpt_web_reverse".to_string(),
        "aistudio_web" => "aistudio_web_reverse".to_string(),
        "gemini_web_chat" => "gemini_web".to_string(),
        "google_gemini_api" | "google_gemini_api_modular" | "aistudio_official_api" => {
            "aistudio_official_api".to_string()
        }
        "google_agent_platform_official_api" | "google_vertex_gemini" | "vertex_official_api" => {
            "google_agent_platform_official_api".to_string()
        }
        "gemini_web_reverse_modular" => "gemini_web_reverse_modular".to_string(),
        "gemini_canvas_web_reverse_modular" => "gemini_canvas_web_reverse_modular".to_string(),
        "gemini_canvas_program_web_reverse_modular" => {
            "gemini_canvas_program_web_reverse_modular".to_string()
        }
        _ => normalized,
    }
}

pub fn default_protocol_profile_for_preset(preset_id: &str) -> &'static str {
    match preset_id.trim() {
        "codex"
        | "chatgpt-codex-oauth-official"
        | "chatgpt-codex-oauth-official-api"
        | "chatgpt-codex-backend" => "chatgpt_codex_oauth_official_api",
        "openai" => "chatgpt_official_api",
        "azure-openai" => "azure_openai",
        "gemini-api" | "aistudio-official-api" => "aistudio_official_api",
        "google-agent-platform"
        | "google-agent-platform-official-api"
        | "vertex-gemini"
        | "vertex-official-api" => "google_agent_platform_official_api",
        "chatgpt-web-reverse" => "chatgpt_web_reverse",
        "aistudio-web-reverse" => "aistudio_web_reverse",
        "gemini-web-chat" => "gemini_web",
        "aistudio" => "aistudio_web_reverse",
        "gemini-api-modular" => "aistudio_official_api",
        "gemini-web-chat-modular" => "gemini_web_reverse_modular",
        "gemini-canvas-browser-relay" => "gemini_canvas_web_reverse_modular",
        "gemini-canvas-program-relay" => "gemini_canvas_program_web_reverse_modular",
        "bedrock-converse" => "aws_bedrock",
        "cohere-chat" => "cohere",
        "groq-openai" => "groq",
        "together-openai" => "together",
        "openrouter-openai" => "openrouter",
        "muyuan-openai" => "muyuan",
        "poe-openai" => "poe",
        "longcat-openai" => "longcat",
        "deepseek-openai" => "deepseek",
        "mistral-openai" => "mistral",
        "xai-openai" => "xai",
        "nvidia-openai" => "nvidia",
        "xfyun" => "xfyun_openai",
        "xfyun-websocket" => "xfyun_native_websocket",
        "anthropic" => "anthropic",
        "accio" => "accio",
        "qwen" => "qwen_dashscope_openai",
        "qwen-dashscope-openai" => "qwen_dashscope_openai",
        "qwen-coding-plan-openai" => "qwen_coding_plan_openai",
        "qwen-coding-plan-anthropic" => "qwen_coding_plan_anthropic",
        "qwen-web"
        | "qwen-webui"
        | "qwen-web-chat"
        | "qwen-webui-replay"
        | "qwen-webui-replay-live" => "qwen_web_chat",
        "chatgpt-web-chat" => "chatgpt_web_reverse",
        "aistudio-web-chat" => "aistudio_web_reverse",
        "grok" => "grok_web",
        "perplexity" => "perplexity_chat",
        "perplexity-search" => "perplexity_search",
        "linkup" => "linkup",
        "tavily" => "tavily",
        "you" => "you_search",
        "exa" => "exa",
        "jina-search" => "jina_search",
        "jina-reader" => "jina_reader",
        "websearchapi" => "websearchapi",
        "gemini-business" => "gemini_business",
        "chataibot" => "chataibot",
        "lumalabs" => "lumalabs",
        "gemini-canvas" => "gemini_canvas",
        "gemini-canvas-chat" => "gemini_canvas",
        "gemini-web" => "gemini_web",
        "kiro" => "kiro",
        "freebuff" => "freebuff",
        "producer" => "producer",
        "suno" => "suno",
        "udio" => "udio",
        _ => "custom",
    }
}

pub fn default_protocol_profile_for_adapter(adapter: &str) -> &'static str {
    match adapter.trim() {
        "accio_compatible" => "accio",
        "anthropic_compatible" => "anthropic",
        "gemini_api_compatible" => "aistudio_official_api",
        "bedrock_converse_compatible" => "aws_bedrock",
        "cohere_compatible" => "cohere",
        "kiro_compatible" => "kiro",
        "freebuff_compatible" => "freebuff",
        "producer_compatible" => "producer",
        "gemini_business_compatible" => "gemini_business",
        "chataibot_compatible" => "chataibot",
        "lumalabs_compatible" => "lumalabs",
        "gemini_canvas_compatible" => "gemini_canvas",
        "gemini_api_modular_compatible" => "aistudio_official_api",
        "gemini_web_reverse_modular_compatible" => "gemini_web_reverse_modular",
        "gemini_canvas_web_reverse_compatible" => "gemini_canvas_web_reverse_modular",
        "gemini_canvas_program_web_reverse_compatible" => {
            "gemini_canvas_program_web_reverse_modular"
        }
        "gemini_web_compatible" => "gemini_web",
        "suno_compatible" => "suno",
        "udio_compatible" => "udio",
        "xfyun_websocket_compatible" => "xfyun_native_websocket",
        "qwen_web_compatible" => "qwen_web_chat",
        "chatgpt_web_reverse_compatible" => "chatgpt_web_reverse",
        "aistudio_web_reverse_compatible" => "aistudio_web_reverse",
        "search_api_compatible" | "linkup_compatible" => "search_generic",
        "openai_compatible" => "openai_compatible_generic",
        _ => "custom",
    }
}

pub fn default_protocol_family_for_adapter(adapter: &str) -> &'static str {
    match adapter.trim() {
        "accio_compatible" => OPENAI_RESPONSES_FAMILY,
        "anthropic_compatible" => "anthropic",
        "gemini_api_compatible" => GEMINI_GENERATE_CONTENT_FAMILY,
        "gemini_api_modular_compatible" => GEMINI_GENERATE_CONTENT_FAMILY,
        "bedrock_converse_compatible" => BEDROCK_CONVERSE_FAMILY,
        "cohere_compatible" => COHERE_CHAT_FAMILY,
        "kiro_compatible" => "kiro",
        "freebuff_compatible" => "freebuff",
        "producer_compatible" => PRODUCER_MUSIC_FAMILY,
        "gemini_business_compatible" => GEMINI_BUSINESS_IMAGES_FAMILY,
        "chataibot_compatible" => CHATAIBOT_IMAGES_FAMILY,
        "lumalabs_compatible" => LUMALABS_IMAGES_FAMILY,
        "gemini_canvas_compatible" => GEMINI_CANVAS_IMAGES_FAMILY,
        "gemini_canvas_web_reverse_compatible" => GEMINI_CANVAS_IMAGES_FAMILY,
        "gemini_canvas_program_web_reverse_compatible" => GEMINI_CANVAS_IMAGES_FAMILY,
        "gemini_web_compatible" => GEMINI_WEB_CHAT_FAMILY,
        "gemini_web_reverse_modular_compatible" => GEMINI_WEB_CHAT_FAMILY,
        "suno_compatible" => SUNO_MUSIC_FAMILY,
        "udio_compatible" => UDIO_MUSIC_FAMILY,
        "xfyun_websocket_compatible" => "xfyun_websocket",
        "qwen_web_compatible" => QWEN_WEB_CHAT_FAMILY,
        "chatgpt_web_reverse_compatible" => CHATGPT_WEB_CHAT_FAMILY,
        "aistudio_web_reverse_compatible" => GEMINI_GENERATE_CONTENT_FAMILY,
        "search_api_compatible" | "linkup_compatible" => SEARCH_API_FAMILY,
        _ => "openai",
    }
}

pub fn default_protocol_family_for_profile(profile: &str, adapter: &str) -> String {
    match canonicalize_protocol_profile_key(profile).as_str() {
        "chatgpt_official_api" | "chatgpt_codex_oauth_official_api" => {
            OPENAI_CHAT_FAMILY.to_string()
        }
        "accio" => OPENAI_RESPONSES_FAMILY.to_string(),
        "qwen_web_chat" => QWEN_WEB_CHAT_FAMILY.to_string(),
        "chatgpt_web_reverse" => CHATGPT_WEB_CHAT_FAMILY.to_string(),
        "aistudio_official_api" | "google_agent_platform_official_api" => {
            GEMINI_GENERATE_CONTENT_FAMILY.to_string()
        }
        "aistudio_web_reverse" => GEMINI_GENERATE_CONTENT_FAMILY.to_string(),
        "perplexity_search" => PERPLEXITY_SEARCH_FAMILY.to_string(),
        "tavily" => TAVILY_SEARCH_FAMILY.to_string(),
        "exa" => EXA_SEARCH_FAMILY.to_string(),
        "jina_search" => JINA_SEARCH_FAMILY.to_string(),
        "jina_reader" => JINA_READER_FAMILY.to_string(),
        "linkup" => LINKUP_SEARCH_FAMILY.to_string(),
        "you_search" => YOU_SEARCH_FAMILY.to_string(),
        "websearchapi" => WEBSEARCHAPI_SEARCH_FAMILY.to_string(),
        "gemini_business" => GEMINI_BUSINESS_IMAGES_FAMILY.to_string(),
        "chataibot" => CHATAIBOT_IMAGES_FAMILY.to_string(),
        "lumalabs" => LUMALABS_IMAGES_FAMILY.to_string(),
        "gemini_canvas" => GEMINI_CANVAS_IMAGES_FAMILY.to_string(),
        "gemini_web" => GEMINI_WEB_CHAT_FAMILY.to_string(),
        "producer" => PRODUCER_MUSIC_FAMILY.to_string(),
        "suno" => SUNO_MUSIC_FAMILY.to_string(),
        "udio" => UDIO_MUSIC_FAMILY.to_string(),
        _ => canonicalize_protocol_family_key(default_protocol_family_for_adapter(adapter)),
    }
}

pub fn infer_protocol_family(
    explicit_family: Option<&str>,
    adapter: &str,
    provider_hint: Option<&str>,
    base_url: Option<&str>,
) -> String {
    if let Some(explicit_family) = explicit_family {
        let canonical = canonicalize_protocol_family_key(explicit_family);
        if canonical != SEARCH_API_FAMILY {
            return canonical;
        }
    }

    let inferred_profile = infer_protocol_profile(adapter, provider_hint, base_url);
    let profile_family = default_protocol_family_for_profile(&inferred_profile, adapter);
    if profile_family != SEARCH_API_FAMILY {
        return profile_family;
    }

    explicit_family
        .map(canonicalize_protocol_family_key)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| {
            canonicalize_protocol_family_key(default_protocol_family_for_adapter(adapter))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn infer_protocol_profile_prefers_explicit_provider_hint() {
        assert_eq!(
            infer_protocol_profile(
                "openai_compatible",
                Some("groq"),
                Some("https://api.openai.com/v1"),
            ),
            "groq"
        );
    }

    #[test]
    fn infer_protocol_profile_uses_base_url_for_azure_and_perplexity() {
        assert_eq!(
            infer_protocol_profile(
                "openai_compatible",
                None,
                Some("https://example.openai.azure.com/openai/v1"),
            ),
            "azure_openai"
        );
        assert_eq!(
            infer_protocol_profile(
                "search_api_compatible",
                None,
                Some("https://api.perplexity.ai"),
            ),
            "perplexity_search"
        );
    }

    #[test]
    fn infer_protocol_profile_supports_qwen_hint_and_official_urls() {
        assert_eq!(
            infer_protocol_profile(
                "openai_compatible",
                Some("qwen"),
                Some("https://api.openai.com/v1"),
            ),
            "qwen_dashscope_openai"
        );
        assert_eq!(
            infer_protocol_profile(
                "openai_compatible",
                None,
                Some("https://dashscope.aliyuncs.com/compatible-mode/v1"),
            ),
            "qwen_dashscope_openai"
        );
        assert_eq!(
            infer_protocol_profile(
                "qwen_web_compatible",
                None,
                Some("https://chat.qwen.ai/api/v1"),
            ),
            "qwen_web_chat"
        );
        assert_eq!(
            infer_protocol_profile(
                "anthropic_compatible",
                None,
                Some("https://coding.dashscope.aliyuncs.com/apps/anthropic"),
            ),
            "qwen_coding_plan_anthropic"
        );
    }

    #[test]
    fn infer_protocol_profile_supports_gemini_web_hint_and_base_url() {
        assert_eq!(
            infer_protocol_profile(
                "gemini_web_compatible",
                Some("gemini_web"),
                Some("https://gemini.google.com"),
            ),
            "gemini_web"
        );
        assert_eq!(
            infer_protocol_profile(
                "gemini_web_compatible",
                None,
                Some("https://gemini.google.com/app"),
            ),
            "gemini_web"
        );
        assert_eq!(
            infer_protocol_family(
                None,
                "gemini_web_compatible",
                Some("gemini_web"),
                Some("https://gemini.google.com/app"),
            ),
            GEMINI_WEB_CHAT_FAMILY
        );
    }

    #[test]
    fn infer_protocol_profile_supports_chatgpt_web_reverse_hint_and_base_url() {
        assert_eq!(
            infer_protocol_profile(
                "chatgpt_web_reverse_compatible",
                Some("chatgpt_web_reverse"),
                Some("https://chatgpt.com"),
            ),
            "chatgpt_web_reverse"
        );
        assert_eq!(
            infer_protocol_profile(
                "chatgpt_web_reverse_compatible",
                None,
                Some("https://chatgpt.com/backend-api/conversation"),
            ),
            "chatgpt_web_reverse"
        );
        assert_eq!(
            infer_protocol_family(
                None,
                "chatgpt_web_reverse_compatible",
                Some("chatgpt_web_reverse"),
                Some("https://chatgpt.com/backend-api/conversation"),
            ),
            CHATGPT_WEB_CHAT_FAMILY
        );
    }

    #[test]
    fn infer_protocol_profile_supports_chatgpt_official_and_codex_backend_lines() {
        assert_eq!(
            infer_protocol_profile(
                "openai_compatible",
                Some("openai"),
                Some("https://api.openai.com/v1"),
            ),
            "chatgpt_official_api"
        );
        assert_eq!(
            infer_protocol_profile("openai_compatible", None, Some("https://api.openai.com/v1"),),
            "chatgpt_official_api"
        );
        assert_eq!(
            infer_protocol_profile(
                "openai_compatible",
                Some("codex"),
                Some("https://chatgpt.com/backend-api/codex"),
            ),
            "chatgpt_codex_oauth_official_api"
        );
        assert_eq!(
            infer_protocol_profile(
                "openai_compatible",
                None,
                Some("https://chatgpt.com/backend-api/codex"),
            ),
            "chatgpt_codex_oauth_official_api"
        );
        assert_eq!(
            infer_protocol_family(
                None,
                "openai_compatible",
                Some("openai"),
                Some("https://api.openai.com/v1"),
            ),
            OPENAI_CHAT_FAMILY
        );
    }

    #[test]
    fn infer_protocol_profile_prefers_strong_chatgpt_base_url_over_legacy_broad_hint() {
        assert_eq!(
            infer_protocol_profile(
                "openai_compatible",
                Some("openai"),
                Some("https://chatgpt.com/backend-api/codex"),
            ),
            "chatgpt_codex_oauth_official_api"
        );
        assert_eq!(
            infer_protocol_profile(
                "openai_compatible",
                Some("codex"),
                Some("https://api.openai.com/v1"),
            ),
            "chatgpt_official_api"
        );
        assert_eq!(
            infer_protocol_profile(
                "openai_compatible",
                Some("openai"),
                Some("https://chatgpt.com/backend-api/conversation"),
            ),
            "chatgpt_web_reverse"
        );
    }

    #[test]
    fn infer_protocol_profile_supports_accio_hint_and_phoenix_url() {
        assert_eq!(
            infer_protocol_profile(
                "accio_compatible",
                Some("accio"),
                Some("https://api.openai.com/v1"),
            ),
            "accio"
        );
        assert_eq!(
            infer_protocol_profile(
                "accio_compatible",
                None,
                Some("https://phoenix-gw.alibaba.com"),
            ),
            "accio"
        );
        assert_eq!(
            infer_protocol_family(
                None,
                "accio_compatible",
                Some("accio"),
                Some("https://phoenix-gw.alibaba.com"),
            ),
            OPENAI_RESPONSES_FAMILY
        );
    }

    #[test]
    fn infer_protocol_profile_supports_nvidia_hint_and_official_url() {
        assert_eq!(
            infer_protocol_profile(
                "openai_compatible",
                Some("nvidia"),
                Some("https://api.openai.com/v1"),
            ),
            "nvidia"
        );
        assert_eq!(
            infer_protocol_profile(
                "openai_compatible",
                Some("nvidia-nim"),
                Some("https://api.openai.com/v1"),
            ),
            "nvidia"
        );
        assert_eq!(
            infer_protocol_profile(
                "openai_compatible",
                None,
                Some("https://integrate.api.nvidia.com"),
            ),
            "nvidia"
        );
    }

    #[test]
    fn protocol_family_selector_matches_generic_openai_and_search_groups() {
        assert!(protocol_family_selector_matches(
            "openai",
            "openai_responses"
        ));
        assert!(protocol_family_selector_matches(
            "openai",
            "openai_embeddings"
        ));
        assert!(protocol_family_selector_matches(
            "search_api",
            "linkup_search"
        ));
        assert!(protocol_family_selector_matches("search", "tavily_search"));
        assert!(!protocol_family_selector_matches(
            "perplexity_search",
            "exa_search"
        ));
    }

    #[test]
    fn infer_protocol_family_preserves_special_provider_native_families() {
        assert_eq!(
            infer_protocol_family(None, "udio_compatible", Some("udio"), None),
            UDIO_MUSIC_FAMILY
        );
        assert_eq!(
            infer_protocol_family(
                None,
                "gemini_canvas_compatible",
                Some("gemini_canvas"),
                None
            ),
            GEMINI_CANVAS_IMAGES_FAMILY
        );
    }

    #[test]
    fn lumalabs_selector_matches_all_native_media_families() {
        assert!(protocol_family_selector_matches(
            "lumalabs",
            LUMALABS_IMAGES_FAMILY,
        ));
        assert!(protocol_family_selector_matches(
            "lumalabs",
            LUMALABS_AUDIO_FAMILY,
        ));
        assert!(protocol_family_selector_matches(
            "lumalabs",
            LUMALABS_VIDEOS_FAMILY,
        ));
    }

    #[test]
    fn udio_selector_matches_all_native_media_families() {
        assert!(protocol_family_selector_matches("udio", UDIO_IMAGES_FAMILY));
        assert!(protocol_family_selector_matches("udio", UDIO_MUSIC_FAMILY));
        assert!(protocol_family_selector_matches("udio", UDIO_VIDEOS_FAMILY));
    }

    #[test]
    fn suno_selector_matches_all_native_media_families() {
        assert!(protocol_family_selector_matches("suno", SUNO_IMAGES_FAMILY));
        assert!(protocol_family_selector_matches("suno", SUNO_MUSIC_FAMILY));
        assert!(protocol_family_selector_matches("suno", SUNO_VIDEOS_FAMILY));
    }

    #[test]
    fn producer_selector_matches_all_native_media_families() {
        assert!(protocol_family_selector_matches(
            "producer",
            PRODUCER_IMAGES_FAMILY
        ));
        assert!(protocol_family_selector_matches(
            "producer",
            PRODUCER_MUSIC_FAMILY
        ));
        assert!(protocol_family_selector_matches(
            "producer",
            PRODUCER_VIDEOS_FAMILY
        ));
    }
}

pub fn infer_protocol_profile(
    adapter: &str,
    provider_hint: Option<&str>,
    base_url: Option<&str>,
) -> String {
    let hinted = provider_hint
        .map(canonicalize_protocol_profile_key)
        .filter(|value| !value.is_empty());
    let hinted_profile = hinted.as_deref().and_then(profile_from_hint);
    let base_url_profile = base_url
        .map(|value| value.trim().to_lowercase())
        .as_deref()
        .and_then(|value| profile_from_base_url(adapter, value));

    match (hinted_profile, base_url_profile) {
        (Some(hinted), Some(base_url_profile))
            if should_prefer_base_url_profile(adapter, hinted, base_url_profile) =>
        {
            return base_url_profile.to_string();
        }
        (Some(hinted), _) => return hinted.to_string(),
        (None, Some(base_url_profile)) => return base_url_profile.to_string(),
        (None, None) => {}
    }
    canonicalize_protocol_profile_key(default_protocol_profile_for_adapter(adapter))
}

fn should_prefer_base_url_profile(
    adapter: &str,
    hinted_profile: &str,
    base_url_profile: &str,
) -> bool {
    hinted_profile != base_url_profile
        && ((is_chatgpt_profile(hinted_profile) && is_chatgpt_profile(base_url_profile))
            || (adapter.trim() == "chatgpt_web_reverse_compatible"
                && base_url_profile == "chatgpt_web_reverse"))
}

fn is_chatgpt_profile(profile: &str) -> bool {
    matches!(
        profile,
        "chatgpt_official_api" | "chatgpt_codex_oauth_official_api" | "chatgpt_web_reverse"
    )
}

fn profile_from_hint(value: &str) -> Option<&'static str> {
    match value {
        "openai" | "openai_platform" | "chatgpt_official_api" => Some("chatgpt_official_api"),
        "codex" | "chatgpt_codex_backend" | "chatgpt_codex_oauth_official_api" => {
            Some("chatgpt_codex_oauth_official_api")
        }
        "azure" | "azure_openai" => Some("azure_openai"),
        "anthropic" => Some("anthropic"),
        "google" | "gemini" | "google_gemini" | "google_gemini_api" | "aistudio_official_api" => {
            Some("aistudio_official_api")
        }
        "vertex"
        | "google_vertex"
        | "google_vertex_gemini"
        | "vertex_official_api"
        | "google_agent_platform"
        | "google_agent_platform_official_api" => Some("google_agent_platform_official_api"),
        "bedrock" | "aws_bedrock" => Some("aws_bedrock"),
        "cohere" => Some("cohere"),
        "groq" => Some("groq"),
        "together" => Some("together"),
        "openrouter" => Some("openrouter"),
        "deepseek" => Some("deepseek"),
        "mistral" => Some("mistral"),
        "xai" | "x_ai" => Some("xai"),
        "nvidia" | "nvidia_platform" | "nvidia_nim" => Some("nvidia"),
        "perplexity" | "perplexity_chat" => Some("perplexity_chat"),
        "perplexity_search" => Some("perplexity_search"),
        "tavily" => Some("tavily"),
        "exa" => Some("exa"),
        "jina" | "jina_search" => Some("jina_search"),
        "jina_reader" => Some("jina_reader"),
        "linkup" => Some("linkup"),
        "you" | "you_search" => Some("you_search"),
        "websearchapi" => Some("websearchapi"),
        "accio" | "accio_platform" => Some("accio"),
        "qwen" | "qwen_platform" => Some("qwen_dashscope_openai"),
        "qwen_dashscope" | "qwen_dashscope_openai" => Some("qwen_dashscope_openai"),
        "qwen_coding_plan" | "qwen_coding_plan_openai" => Some("qwen_coding_plan_openai"),
        "qwen_coding_plan_anthropic" => Some("qwen_coding_plan_anthropic"),
        "qwen_web" | "qwen_web_chat" => Some("qwen_web_chat"),
        "aistudio" | "aistudio_web" | "aistudio_web_reverse" => Some("aistudio_web_reverse"),
        "chatgpt_web" | "chatgpt_web_reverse" | "chatgpt_web_chat" => Some("chatgpt_web_reverse"),
        "gemini_web" | "gemini_web_chat" => Some("gemini_web"),
        "gemini_web_reverse_modular" => Some("gemini_web_reverse_modular"),
        "gemini_canvas_web_reverse_modular" => Some("gemini_canvas_web_reverse_modular"),
        "gemini_canvas_program_web_reverse_modular" => {
            Some("gemini_canvas_program_web_reverse_modular")
        }
        "kiro" => Some("kiro"),
        "freebuff" => Some("freebuff"),
        "producer" => Some("producer"),
        "gemini_canvas" => Some("gemini_canvas"),
        "suno" => Some("suno"),
        "udio" => Some("udio"),
        "xfyun" | "xfyun_openai" => Some("xfyun_openai"),
        "xfyun_native_websocket" => Some("xfyun_native_websocket"),
        _ => None,
    }
}

fn profile_from_base_url(adapter: &str, base_url: &str) -> Option<&'static str> {
    if base_url.contains("openai.azure.com")
        || (base_url.contains("azure.com") && base_url.contains("/openai/"))
        || base_url.contains(".cognitiveservices.azure.com")
    {
        return Some("azure_openai");
    }
    if base_url.contains("chatgpt.com/backend-api/codex") {
        return Some("chatgpt_codex_oauth_official_api");
    }
    if crate::protocol::chatgpt::official_api::is_chatgpt_official_api_base_url(base_url) {
        return Some("chatgpt_official_api");
    }
    if base_url.contains("chatgpt.com/backend-api/conversation")
        || base_url.contains("chatgpt.com/backend-api/models")
        || (adapter.trim() == "chatgpt_web_reverse_compatible" && base_url.contains("chatgpt.com"))
    {
        return Some("chatgpt_web_reverse");
    }
    if base_url.contains("phoenix-gw.alibaba.com") || base_url.contains("accio.com") {
        return Some("accio");
    }
    if base_url.contains("chat.qwen.ai") {
        return Some("qwen_web_chat");
    }
    if base_url.contains("ai.studio") || base_url.contains("aistudio.google.com") {
        return Some("aistudio_web_reverse");
    }
    if base_url.contains("coding.dashscope.aliyuncs.com/apps/anthropic") {
        return Some("qwen_coding_plan_anthropic");
    }
    if base_url.contains("coding.dashscope.aliyuncs.com") {
        return Some("qwen_coding_plan_openai");
    }
    if base_url.contains("dashscope.aliyuncs.com/compatible-mode/")
        || base_url.contains("dashscope-us.aliyuncs.com/compatible-mode/")
        || base_url.contains("dashscope-intl.aliyuncs.com/compatible-mode/")
    {
        return Some("qwen_dashscope_openai");
    }
    if base_url.contains("api.anthropic.com") {
        return Some("anthropic");
    }
    if base_url.contains("generativelanguage.googleapis.com") {
        return Some("aistudio_official_api");
    }
    if base_url.contains("aiplatform.googleapis.com")
        || base_url.contains("vertexai.googleapis.com")
        || base_url.contains("/publishers/google/models/")
    {
        return Some("google_agent_platform_official_api");
    }
    if base_url.contains("bedrock") && base_url.contains("amazonaws.com") {
        return Some("aws_bedrock");
    }
    if base_url.contains("api.cohere.ai") {
        return Some("cohere");
    }
    if base_url.contains("api.groq.com") || base_url.contains("console.groq.com") {
        return Some("groq");
    }
    if base_url.contains("api.together.xyz") || base_url.contains("api.together.ai") {
        return Some("together");
    }
    if base_url.contains("openrouter.ai") {
        return Some("openrouter");
    }
    if base_url.contains("muyuan.do") {
        return Some("muyuan");
    }
    if base_url.contains("api.poe.com") {
        return Some("poe");
    }
    if base_url.contains("api.longcat.chat") {
        return Some("longcat");
    }
    if base_url.contains("api.deepseek.com") {
        return Some("deepseek");
    }
    if base_url.contains("api.mistral.ai") {
        return Some("mistral");
    }
    if base_url.contains("api.x.ai") || base_url.contains("x.ai") {
        return Some("xai");
    }
    if base_url.contains("integrate.api.nvidia.com") || base_url.contains("api.nvidia.com") {
        return Some("nvidia");
    }
    if base_url.contains("api.perplexity.ai") {
        return Some(if adapter.trim() == "search_api_compatible" {
            "perplexity_search"
        } else {
            "perplexity_chat"
        });
    }
    if base_url.contains("api.tavily.com") {
        return Some("tavily");
    }
    if base_url.contains("api.exa.ai") {
        return Some("exa");
    }
    if base_url.contains("api.jina.ai") {
        return Some(if base_url.contains("reader") {
            "jina_reader"
        } else {
            "jina_search"
        });
    }
    if base_url.contains("api.linkup.so") {
        return Some("linkup");
    }
    if base_url.contains("api.ydc-index.io") || base_url.contains("you.com") {
        return Some("you_search");
    }
    if base_url.contains("websearchapi") {
        return Some("websearchapi");
    }
    if base_url.contains("business.gemini.google")
        || base_url.contains("biz-discoveryengine.googleapis.com")
    {
        return Some("gemini_business");
    }
    if base_url.contains("chataibot.pro") {
        return Some("chataibot");
    }
    if base_url.contains("lumalabs.ai") {
        return Some("lumalabs");
    }
    if adapter.trim() == "gemini_web_compatible" && base_url.contains("gemini.google.com") {
        return Some("gemini_web");
    }
    if base_url.contains("gemini.google.com") {
        return Some("gemini_canvas");
    }
    if base_url.contains("producer.ai") {
        return Some("producer");
    }
    if base_url.contains("suno.com") {
        return Some("suno");
    }
    if base_url.contains("udio.com") {
        return Some("udio");
    }
    None
}

#[cfg(test)]
mod official_profile_tests {
    use super::*;

    #[test]
    fn canonicalize_google_official_aliases_to_new_profiles() {
        assert_eq!(
            canonicalize_protocol_profile_key("google_gemini_api"),
            "aistudio_official_api"
        );
        assert_eq!(
            canonicalize_protocol_profile_key("google_gemini_api_modular"),
            "aistudio_official_api"
        );
        assert_eq!(
            canonicalize_protocol_profile_key("google_vertex_gemini"),
            "google_agent_platform_official_api"
        );
        assert_eq!(
            canonicalize_protocol_profile_key("aistudio-official-api"),
            "aistudio_official_api"
        );
        assert_eq!(
            canonicalize_protocol_profile_key("vertex-official-api"),
            "google_agent_platform_official_api"
        );
        assert_eq!(
            canonicalize_protocol_profile_key("google-agent-platform-official-api"),
            "google_agent_platform_official_api"
        );
    }

    #[test]
    fn canonicalize_qwen_webui_replay_aliases_to_qwen_web_chat() {
        assert_eq!(
            canonicalize_protocol_profile_key("qwen-web"),
            "qwen_web_chat"
        );
        assert_eq!(
            canonicalize_protocol_profile_key("qwen-webui"),
            "qwen_web_chat"
        );
        assert_eq!(
            canonicalize_protocol_profile_key("qwen-webui-replay"),
            "qwen_web_chat"
        );
        assert_eq!(
            canonicalize_protocol_profile_key("qwen-webui-replay-live"),
            "qwen_web_chat"
        );
    }

    #[test]
    fn default_qwen_preset_aliases_map_to_canonical_profiles() {
        assert_eq!(
            default_protocol_profile_for_preset("qwen-web"),
            "qwen_web_chat"
        );
        assert_eq!(
            default_protocol_profile_for_preset("qwen-webui"),
            "qwen_web_chat"
        );
        assert_eq!(
            default_protocol_profile_for_preset("qwen-webui-replay"),
            "qwen_web_chat"
        );
        assert_eq!(
            default_protocol_profile_for_preset("qwen-webui-replay-live"),
            "qwen_web_chat"
        );
    }

    #[test]
    fn infer_profile_prefers_new_google_official_profiles_from_base_urls() {
        assert_eq!(
            infer_protocol_profile(
                "gemini_api_compatible",
                None,
                Some("https://generativelanguage.googleapis.com/v1beta")
            ),
            "aistudio_official_api"
        );
        assert_eq!(
            infer_protocol_profile(
                "gemini_api_compatible",
                None,
                Some(
                    "https://aiplatform.googleapis.com/v1/projects/demo/locations/us-central1/publishers/google/models"
                )
            ),
            "google_agent_platform_official_api"
        );
    }

    #[test]
    fn line_compile_switches_map_profiles_and_adapters() {
        assert_eq!(
            crate::implementation_lines::required_feature_for_protocol_profile(
                "chatgpt_official_api"
            ),
            Some("line-chatgpt-official-api")
        );
        assert_eq!(
            crate::implementation_lines::required_feature_for_protocol_profile(
                "chatgpt_codex_oauth_official_api"
            ),
            Some("line-chatgpt-codex-oauth-official")
        );
        assert_eq!(
            crate::implementation_lines::required_feature_for_protocol_profile(
                "chatgpt_web_reverse"
            ),
            Some("line-chatgpt-web-reverse")
        );
        assert_eq!(
            crate::implementation_lines::required_feature_for_protocol_profile("nvidia"),
            Some("line-nvidia-openai-official-vendor-api")
        );
        assert_eq!(
            crate::implementation_lines::required_feature_for_protocol_profile("grok_web"),
            Some("line-grok-web-reverse-api")
        );
        assert_eq!(
            crate::implementation_lines::required_feature_for_protocol_profile(
                "aistudio_official_api"
            ),
            Some("line-aistudio-official")
        );
        assert_eq!(
            crate::implementation_lines::required_feature_for_protocol_profile(
                "google_agent_platform_official_api"
            ),
            Some("line-google-agent-platform-official")
        );
        assert_eq!(
            crate::implementation_lines::required_feature_for_protocol_profile(
                "gemini_web_reverse_modular"
            ),
            Some("line-gemini-web-reverse")
        );
        assert_eq!(
            crate::implementation_lines::required_feature_for_protocol_profile(
                "gemini_canvas_program_web_reverse_modular"
            ),
            Some("line-gemini-canvas-program")
        );
        assert_eq!(
            crate::implementation_lines::required_feature_for_protocol_profile(
                "aistudio_web_reverse"
            ),
            Some("line-aistudio-web-reverse")
        );
        assert_eq!(
            crate::implementation_lines::required_feature_for_protocol_profile(
                "qwen_dashscope_openai"
            ),
            Some("line-qwen-official-api")
        );
        assert_eq!(
            crate::implementation_lines::required_feature_for_protocol_profile(
                "qwen_coding_plan_openai"
            ),
            Some("line-qwen-official-api")
        );
        assert_eq!(
            crate::implementation_lines::required_feature_for_protocol_profile(
                "qwen_coding_plan_anthropic"
            ),
            Some("line-qwen-official-api")
        );
        assert_eq!(
            crate::implementation_lines::required_feature_for_protocol_profile("qwen_web_chat"),
            Some("line-qwen-web-reverse")
        );

        assert_eq!(
            crate::implementation_lines::required_feature_for_adapter(
                "gemini_web_reverse_modular_compatible"
            ),
            Some("line-gemini-web-reverse")
        );
        assert_eq!(
            crate::implementation_lines::required_feature_for_adapter(
                "gemini_canvas_program_web_reverse_compatible"
            ),
            Some("line-gemini-canvas-program")
        );
        assert_eq!(
            crate::implementation_lines::required_feature_for_adapter(
                "aistudio_web_reverse_compatible"
            ),
            Some("line-aistudio-web-reverse")
        );
        assert_eq!(
            crate::implementation_lines::required_feature_for_adapter("qwen_web_compatible"),
            Some("line-qwen-web-reverse")
        );
        assert_eq!(
            crate::implementation_lines::required_feature_for_adapter("grok_compatible"),
            Some("line-grok-web-reverse-api")
        );
    }

    #[test]
    fn default_build_keeps_nvidia_and_grok_lines_enabled() {
        assert!(crate::implementation_lines::is_protocol_profile_compiled_in("nvidia"));
        assert!(crate::implementation_lines::is_protocol_profile_compiled_in("grok_web"));
    }

    #[test]
    fn default_build_keeps_all_ten_refactored_lines_enabled() {
        assert!(
            crate::implementation_lines::is_protocol_profile_compiled_in("chatgpt_official_api")
        );
        assert!(
            crate::implementation_lines::is_protocol_profile_compiled_in(
                "chatgpt_codex_oauth_official_api"
            )
        );
        assert!(
            crate::implementation_lines::is_protocol_profile_compiled_in("chatgpt_web_reverse")
        );
        assert!(
            crate::implementation_lines::is_protocol_profile_compiled_in("aistudio_official_api")
        );
        assert!(
            crate::implementation_lines::is_protocol_profile_compiled_in(
                "google_agent_platform_official_api"
            )
        );
        assert!(
            crate::implementation_lines::is_protocol_profile_compiled_in("aistudio_web_reverse")
        );
        assert!(
            crate::implementation_lines::is_protocol_profile_compiled_in(
                "gemini_web_reverse_modular"
            )
        );
        assert!(
            crate::implementation_lines::is_protocol_profile_compiled_in(
                "gemini_canvas_program_web_reverse_modular"
            )
        );
        assert!(
            crate::implementation_lines::is_protocol_profile_compiled_in("qwen_dashscope_openai")
        );
        assert!(
            crate::implementation_lines::is_protocol_profile_compiled_in("qwen_coding_plan_openai")
        );
        assert!(
            crate::implementation_lines::is_protocol_profile_compiled_in(
                "qwen_coding_plan_anthropic"
            )
        );
        assert!(crate::implementation_lines::is_protocol_profile_compiled_in("qwen_web_chat"));
    }
}
