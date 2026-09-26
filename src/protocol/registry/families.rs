//! Protocol-family canonical names, aliases and family selectors.

use super::ANTHROPIC_MESSAGES_FAMILY;
use super::BEDROCK_CONVERSE_FAMILY;
use super::CHATAIBOT_IMAGES_FAMILY;
use super::CHATGPT_WEB_CHAT_FAMILY;
use super::COHERE_CHAT_FAMILY;
use super::COHERE_CHAT_V2_FAMILY_ALIAS;
use super::EXA_SEARCH_FAMILY;
use super::GEMINI_BUSINESS_IMAGES_FAMILY;
use super::GEMINI_CANVAS_IMAGES_FAMILY;
use super::GEMINI_CANVAS_MUSIC_FAMILY;
use super::GEMINI_CANVAS_VIDEOS_FAMILY;
use super::GEMINI_CANVAS_WEB_RELAY_FAMILY;
use super::GEMINI_GENERATE_CONTENT_FAMILY;
use super::GEMINI_LIVE_FAMILY;
use super::GEMINI_WEB_CHAT_FAMILY;
use super::JINA_READER_FAMILY;
use super::JINA_SEARCH_FAMILY;
use super::LINKUP_SEARCH_FAMILY;
use super::LUMALABS_AUDIO_FAMILY;
use super::LUMALABS_IMAGES_FAMILY;
use super::LUMALABS_VIDEOS_FAMILY;
use super::OPENAI_AUDIO_SPEECH_FAMILY;
use super::OPENAI_AUDIO_TRANSCRIPTIONS_FAMILY;
use super::OPENAI_CHAT_FAMILY;
use super::OPENAI_EMBEDDINGS_FAMILY;
use super::OPENAI_IMAGES_EDITS_FAMILY;
use super::OPENAI_IMAGES_GENERATIONS_FAMILY;
use super::OPENAI_LEGACY_COMPLETIONS_FAMILY;
use super::OPENAI_MUSIC_GENERATIONS_FAMILY;
use super::OPENAI_REALTIME_FAMILY;
use super::OPENAI_RESPONSES_FAMILY;
use super::OPENAI_VIDEOS_GENERATIONS_FAMILY;
use super::PERPLEXITY_SEARCH_FAMILY;
use super::PRODUCER_IMAGES_FAMILY;
use super::PRODUCER_MUSIC_FAMILY;
use super::PRODUCER_VIDEOS_FAMILY;
use super::QWEN_WEB_CHAT_FAMILY;
use super::SEARCH_API_FAMILY;
use super::SUNO_IMAGES_FAMILY;
use super::SUNO_MUSIC_FAMILY;
use super::SUNO_VIDEOS_FAMILY;
use super::TAVILY_SEARCH_FAMILY;
use super::UDIO_IMAGES_FAMILY;
use super::UDIO_MUSIC_FAMILY;
use super::UDIO_VIDEOS_FAMILY;
use super::WEBSEARCHAPI_SEARCH_FAMILY;
use super::YOU_SEARCH_FAMILY;

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
