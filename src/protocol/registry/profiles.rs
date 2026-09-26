//! Protocol-profile aliases and defaults for presets and adapters.

use super::families::canonicalize_protocol_family_key;
use super::BEDROCK_CONVERSE_FAMILY;
use super::CHATAIBOT_IMAGES_FAMILY;
use super::CHATGPT_WEB_CHAT_FAMILY;
use super::COHERE_CHAT_FAMILY;
use super::EXA_SEARCH_FAMILY;
use super::GEMINI_BUSINESS_IMAGES_FAMILY;
use super::GEMINI_CANVAS_IMAGES_FAMILY;
use super::GEMINI_GENERATE_CONTENT_FAMILY;
use super::GEMINI_WEB_CHAT_FAMILY;
use super::JINA_READER_FAMILY;
use super::JINA_SEARCH_FAMILY;
use super::LINKUP_SEARCH_FAMILY;
use super::LUMALABS_IMAGES_FAMILY;
use super::OPENAI_CHAT_FAMILY;
use super::OPENAI_RESPONSES_FAMILY;
use super::PERPLEXITY_SEARCH_FAMILY;
use super::PRODUCER_MUSIC_FAMILY;
use super::QWEN_WEB_CHAT_FAMILY;
use super::SEARCH_API_FAMILY;
use super::SUNO_MUSIC_FAMILY;
use super::TAVILY_SEARCH_FAMILY;
use super::UDIO_MUSIC_FAMILY;
use super::WEBSEARCHAPI_SEARCH_FAMILY;
use super::YOU_SEARCH_FAMILY;

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
