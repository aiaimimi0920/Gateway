//! Caller wire-family and adapter surface capability projection.

use crate::protocol::canonical::CanonicalRelayRequest;
use crate::protocol::canonical::EndpointKind;
use crate::protocol::canonical::ProtocolFamily;
use crate::protocol::registry::canonicalize_wire_protocol_family_key;
use crate::protocol::registry::ANTHROPIC_MESSAGES_FAMILY;
use crate::protocol::registry::BEDROCK_CONVERSE_FAMILY;
use crate::protocol::registry::CHATAIBOT_IMAGES_FAMILY;
use crate::protocol::registry::CHATGPT_WEB_CHAT_FAMILY;
use crate::protocol::registry::COHERE_CHAT_FAMILY;
use crate::protocol::registry::GEMINI_BUSINESS_IMAGES_FAMILY;
use crate::protocol::registry::GEMINI_CANVAS_IMAGES_FAMILY;
use crate::protocol::registry::GEMINI_CANVAS_MUSIC_FAMILY;
use crate::protocol::registry::GEMINI_CANVAS_VIDEOS_FAMILY;
use crate::protocol::registry::GEMINI_GENERATE_CONTENT_FAMILY;
use crate::protocol::registry::GEMINI_LIVE_FAMILY;
use crate::protocol::registry::GEMINI_WEB_CHAT_FAMILY;
use crate::protocol::registry::LUMALABS_AUDIO_FAMILY;
use crate::protocol::registry::LUMALABS_IMAGES_FAMILY;
use crate::protocol::registry::LUMALABS_VIDEOS_FAMILY;
use crate::protocol::registry::OPENAI_AUDIO_SPEECH_FAMILY;
use crate::protocol::registry::OPENAI_AUDIO_TRANSCRIPTIONS_FAMILY;
use crate::protocol::registry::OPENAI_CHAT_FAMILY;
use crate::protocol::registry::OPENAI_EMBEDDINGS_FAMILY;
use crate::protocol::registry::OPENAI_IMAGES_EDITS_FAMILY;
use crate::protocol::registry::OPENAI_IMAGES_GENERATIONS_FAMILY;
use crate::protocol::registry::OPENAI_LEGACY_COMPLETIONS_FAMILY;
use crate::protocol::registry::OPENAI_MUSIC_GENERATIONS_FAMILY;
use crate::protocol::registry::OPENAI_REALTIME_FAMILY;
use crate::protocol::registry::OPENAI_RESPONSES_FAMILY;
use crate::protocol::registry::OPENAI_VIDEOS_GENERATIONS_FAMILY;
use crate::protocol::registry::PRODUCER_IMAGES_FAMILY;
use crate::protocol::registry::PRODUCER_MUSIC_FAMILY;
use crate::protocol::registry::PRODUCER_VIDEOS_FAMILY;
use crate::protocol::registry::SEARCH_API_FAMILY;
use crate::protocol::registry::SUNO_IMAGES_FAMILY;
use crate::protocol::registry::SUNO_MUSIC_FAMILY;
use crate::protocol::registry::SUNO_VIDEOS_FAMILY;
use crate::protocol::registry::UDIO_IMAGES_FAMILY;
use crate::protocol::registry::UDIO_MUSIC_FAMILY;
use crate::protocol::registry::UDIO_VIDEOS_FAMILY;
use crate::routing::candidate::canonicalize_adapter_name;

pub fn requested_wire_protocol_family(req: &CanonicalRelayRequest) -> Option<String> {
    match req.protocol_family {
        ProtocolFamily::OpenAi => Some(match req.endpoint_kind {
            EndpointKind::Responses => OPENAI_RESPONSES_FAMILY,
            EndpointKind::Completions => OPENAI_LEGACY_COMPLETIONS_FAMILY,
            EndpointKind::Embeddings => OPENAI_EMBEDDINGS_FAMILY,
            EndpointKind::AudioTranscriptions => OPENAI_AUDIO_TRANSCRIPTIONS_FAMILY,
            EndpointKind::AudioSpeech => OPENAI_AUDIO_SPEECH_FAMILY,
            EndpointKind::ImagesGenerations => OPENAI_IMAGES_GENERATIONS_FAMILY,
            EndpointKind::ImagesEdits => OPENAI_IMAGES_EDITS_FAMILY,
            EndpointKind::MusicGenerations => OPENAI_MUSIC_GENERATIONS_FAMILY,
            EndpointKind::VideosGenerations => OPENAI_VIDEOS_GENERATIONS_FAMILY,
            _ => OPENAI_CHAT_FAMILY,
        }),
        ProtocolFamily::OpenAiRealtime => Some(OPENAI_REALTIME_FAMILY),
        ProtocolFamily::Anthropic => Some(ANTHROPIC_MESSAGES_FAMILY),
        ProtocolFamily::GeminiGenerateContent => Some(GEMINI_GENERATE_CONTENT_FAMILY),
        ProtocolFamily::GeminiLive => Some(GEMINI_LIVE_FAMILY),
        ProtocolFamily::BedrockConverse => Some(BEDROCK_CONVERSE_FAMILY),
        ProtocolFamily::CohereChat => Some(COHERE_CHAT_FAMILY),
        ProtocolFamily::DashScope => Some("dashscope_text"),
        ProtocolFamily::DashScopeMultimodal => Some("dashscope_multimodal"),
        ProtocolFamily::SearchApi => Some(SEARCH_API_FAMILY),
    }
    .map(str::to_string)
}

pub fn surface_supported_wire_protocol_families(
    adapter: &str,
    fallback_protocol_family: &str,
) -> Vec<String> {
    match canonicalize_adapter_name(adapter).as_str() {
        "openai_compatible" => vec![
            OPENAI_CHAT_FAMILY.to_string(),
            OPENAI_LEGACY_COMPLETIONS_FAMILY.to_string(),
            OPENAI_RESPONSES_FAMILY.to_string(),
            OPENAI_EMBEDDINGS_FAMILY.to_string(),
            OPENAI_AUDIO_TRANSCRIPTIONS_FAMILY.to_string(),
            OPENAI_AUDIO_SPEECH_FAMILY.to_string(),
        ],
        "freebuff_compatible" | "grok_compatible" => vec![OPENAI_CHAT_FAMILY.to_string()],
        "accio_compatible" | "kiro_compatible" => vec![OPENAI_RESPONSES_FAMILY.to_string()],
        "chatgpt_web_reverse_compatible" => vec![CHATGPT_WEB_CHAT_FAMILY.to_string()],
        "anthropic_compatible" => vec![ANTHROPIC_MESSAGES_FAMILY.to_string()],
        "gemini_api_compatible" | "gemini_api_modular_compatible" => vec![
            GEMINI_GENERATE_CONTENT_FAMILY.to_string(),
            GEMINI_CANVAS_IMAGES_FAMILY.to_string(),
            GEMINI_CANVAS_MUSIC_FAMILY.to_string(),
            GEMINI_CANVAS_VIDEOS_FAMILY.to_string(),
        ],
        "aistudio_web_reverse_compatible" => vec![
            GEMINI_GENERATE_CONTENT_FAMILY.to_string(),
            OPENAI_EMBEDDINGS_FAMILY.to_string(),
            OPENAI_AUDIO_SPEECH_FAMILY.to_string(),
        ],
        "bedrock_converse_compatible" => vec![BEDROCK_CONVERSE_FAMILY.to_string()],
        "cohere_compatible" => vec![COHERE_CHAT_FAMILY.to_string()],
        "dashscope_compatible" => vec!["dashscope_text".into()],
        "dashscope_multimodal_compatible" => vec!["dashscope_multimodal".into()],
        "gemini_business_compatible" => vec![GEMINI_BUSINESS_IMAGES_FAMILY.to_string()],
        "chataibot_compatible" => vec![CHATAIBOT_IMAGES_FAMILY.to_string()],
        "lumalabs_compatible" => vec![
            LUMALABS_IMAGES_FAMILY.to_string(),
            LUMALABS_AUDIO_FAMILY.to_string(),
            LUMALABS_VIDEOS_FAMILY.to_string(),
        ],
        "gemini_canvas_compatible"
        | "gemini_canvas_web_reverse_compatible"
        | "gemini_canvas_program_web_reverse_compatible" => vec![
            GEMINI_GENERATE_CONTENT_FAMILY.to_string(),
            GEMINI_CANVAS_IMAGES_FAMILY.to_string(),
            GEMINI_CANVAS_MUSIC_FAMILY.to_string(),
            GEMINI_CANVAS_VIDEOS_FAMILY.to_string(),
        ],
        "gemini_web_reverse_modular_compatible" => vec![
            GEMINI_WEB_CHAT_FAMILY.to_string(),
            GEMINI_GENERATE_CONTENT_FAMILY.to_string(),
            GEMINI_CANVAS_IMAGES_FAMILY.to_string(),
            GEMINI_CANVAS_MUSIC_FAMILY.to_string(),
            GEMINI_CANVAS_VIDEOS_FAMILY.to_string(),
        ],
        "producer_compatible" => vec![
            PRODUCER_IMAGES_FAMILY.to_string(),
            PRODUCER_MUSIC_FAMILY.to_string(),
            PRODUCER_VIDEOS_FAMILY.to_string(),
        ],
        "suno_compatible" => vec![
            SUNO_IMAGES_FAMILY.to_string(),
            SUNO_MUSIC_FAMILY.to_string(),
            SUNO_VIDEOS_FAMILY.to_string(),
        ],
        "udio_compatible" => vec![
            UDIO_IMAGES_FAMILY.to_string(),
            UDIO_MUSIC_FAMILY.to_string(),
            UDIO_VIDEOS_FAMILY.to_string(),
        ],
        "xfyun_websocket_compatible" => vec!["xfyun_websocket".to_string()],
        "search_api_compatible" => {
            vec![canonicalize_wire_protocol_family_key(
                fallback_protocol_family,
            )]
        }
        _ => vec![canonicalize_wire_protocol_family_key(
            fallback_protocol_family,
        )],
    }
}
