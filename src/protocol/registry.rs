//! Stable protocol-family identifiers and the public resolution API.

mod families;
mod profile_inference;
mod profiles;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod official_profile_tests;

pub use families::{
    canonicalize_protocol_family_key, canonicalize_wire_protocol_family_key,
    is_chatgpt_web_protocol_family, is_gemini_canvas_protocol_family,
    is_gemini_web_protocol_family, is_lumalabs_protocol_family, is_openai_protocol_family,
    is_producer_protocol_family, is_search_protocol_family, is_suno_protocol_family,
    is_udio_protocol_family, protocol_family_selector_matches,
};

pub use profiles::{
    canonicalize_protocol_profile_key, default_protocol_family_for_adapter,
    default_protocol_family_for_profile, default_protocol_profile_for_adapter,
    default_protocol_profile_for_preset,
};

pub use profile_inference::infer_protocol_profile;

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
