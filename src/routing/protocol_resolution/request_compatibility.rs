//! Request endpoint compatibility with the configured upstream wire.

use crate::protocol::canonical::EndpointKind;
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
use crate::protocol::registry::GEMINI_WEB_CHAT_FAMILY;
use crate::protocol::registry::LUMALABS_AUDIO_FAMILY;
use crate::protocol::registry::LUMALABS_IMAGES_FAMILY;
use crate::protocol::registry::LUMALABS_VIDEOS_FAMILY;
use crate::protocol::registry::OPENAI_AUDIO_SPEECH_FAMILY;
use crate::protocol::registry::OPENAI_AUDIO_TRANSCRIPTIONS_FAMILY;
use crate::protocol::registry::OPENAI_CHAT_FAMILY;
use crate::protocol::registry::OPENAI_EMBEDDINGS_FAMILY;
use crate::protocol::registry::OPENAI_LEGACY_COMPLETIONS_FAMILY;
use crate::protocol::registry::OPENAI_RESPONSES_FAMILY;
use crate::protocol::registry::PRODUCER_IMAGES_FAMILY;
use crate::protocol::registry::PRODUCER_MUSIC_FAMILY;
use crate::protocol::registry::PRODUCER_VIDEOS_FAMILY;
use crate::protocol::registry::SUNO_IMAGES_FAMILY;
use crate::protocol::registry::SUNO_MUSIC_FAMILY;
use crate::protocol::registry::SUNO_VIDEOS_FAMILY;
use crate::protocol::registry::UDIO_IMAGES_FAMILY;
use crate::protocol::registry::UDIO_MUSIC_FAMILY;
use crate::protocol::registry::UDIO_VIDEOS_FAMILY;
use crate::routing::candidate::ProviderAccountPayload;

pub(super) fn request_compatible_wire_protocol_families(
    payload: &ProviderAccountPayload,
    endpoint_kind: EndpointKind,
    fallback_protocol_family: &str,
) -> Vec<String> {
    match payload.canonical_adapter() {
        "dashscope_compatible" | "dashscope_multimodal_compatible" => match endpoint_kind {
            EndpointKind::ChatCompletions
            | EndpointKind::Completions
            | EndpointKind::Messages
            | EndpointKind::Responses => vec![if payload.adapter == "dashscope_compatible" {
                "dashscope_text"
            } else {
                "dashscope_multimodal"
            }
            .into()],
            _ => Vec::new(),
        },
        "openai_compatible" => {
            if payload.bridges_openai_text_endpoint_to_responses(endpoint_kind) {
                vec![OPENAI_RESPONSES_FAMILY.to_string()]
            } else if payload.bridges_openai_responses_to_chat_completions(endpoint_kind) {
                vec![OPENAI_CHAT_FAMILY.to_string()]
            } else if endpoint_kind == EndpointKind::Responses {
                vec![OPENAI_RESPONSES_FAMILY.to_string()]
            } else if endpoint_kind == EndpointKind::Embeddings {
                vec![OPENAI_EMBEDDINGS_FAMILY.to_string()]
            } else if endpoint_kind == EndpointKind::AudioTranscriptions {
                vec![OPENAI_AUDIO_TRANSCRIPTIONS_FAMILY.to_string()]
            } else if endpoint_kind == EndpointKind::AudioSpeech {
                vec![OPENAI_AUDIO_SPEECH_FAMILY.to_string()]
            } else if matches!(
                endpoint_kind,
                EndpointKind::ChatCompletions | EndpointKind::Completions | EndpointKind::Messages
            ) {
                if endpoint_kind == EndpointKind::Completions && payload.completions_path.is_some()
                {
                    vec![OPENAI_LEGACY_COMPLETIONS_FAMILY.to_string()]
                } else {
                    vec![OPENAI_CHAT_FAMILY.to_string()]
                }
            } else {
                Vec::new()
            }
        }
        "freebuff_compatible" => match endpoint_kind {
            // FreeBuff's native upstream wire remains OpenAI chat, but this surface is
            // expected to serve `/v1/messages`, `/v1/responses`, and legacy
            // `/v1/completions` through canonical bridge repacking.
            EndpointKind::ChatCompletions
            | EndpointKind::Completions
            | EndpointKind::Messages
            | EndpointKind::Responses => vec![OPENAI_CHAT_FAMILY.to_string()],
            _ => Vec::new(),
        },
        "grok_compatible" => match endpoint_kind {
            EndpointKind::ChatCompletions => vec![OPENAI_CHAT_FAMILY.to_string()],
            _ => Vec::new(),
        },
        "accio_compatible" | "kiro_compatible" => match endpoint_kind {
            // Accio and Kiro both speak a responses-style upstream wire. The
            // gateway should still admit compact OpenAI/Anthropic text ingress
            // and bridge it onto that upstream responses family.
            EndpointKind::ChatCompletions
            | EndpointKind::Completions
            | EndpointKind::Messages
            | EndpointKind::Responses => vec![OPENAI_RESPONSES_FAMILY.to_string()],
            _ => Vec::new(),
        },
        "chatgpt_web_reverse_compatible" => match endpoint_kind {
            EndpointKind::ChatCompletions
            | EndpointKind::Completions
            | EndpointKind::Messages
            | EndpointKind::Responses => vec![CHATGPT_WEB_CHAT_FAMILY.to_string()],
            _ => Vec::new(),
        },
        "anthropic_compatible" => match endpoint_kind {
            // Anthropic-native surfaces can still serve OpenAI/Gemini/Bedrock/Cohere-style
            // ingress after canonical normalization; the gateway bridges those caller-visible
            // protocols onto the Anthropic Messages wire family.
            EndpointKind::ChatCompletions
            | EndpointKind::Completions
            | EndpointKind::Messages
            | EndpointKind::Responses => vec![ANTHROPIC_MESSAGES_FAMILY.to_string()],
            _ => Vec::new(),
        },
        "gemini_api_compatible" | "gemini_api_modular_compatible" => match endpoint_kind {
            EndpointKind::ChatCompletions
            | EndpointKind::Messages
            | EndpointKind::Responses
            | EndpointKind::Completions
            | EndpointKind::AudioSpeech => {
                vec![GEMINI_GENERATE_CONTENT_FAMILY.to_string()]
            }
            EndpointKind::ImagesGenerations => vec![GEMINI_CANVAS_IMAGES_FAMILY.to_string()],
            EndpointKind::MusicGenerations => vec![GEMINI_CANVAS_MUSIC_FAMILY.to_string()],
            EndpointKind::VideosGenerations => vec![GEMINI_CANVAS_VIDEOS_FAMILY.to_string()],
            _ => Vec::new(),
        },
        "aistudio_web_reverse_compatible" => match endpoint_kind {
            EndpointKind::ChatCompletions
            | EndpointKind::Messages
            | EndpointKind::Responses
            | EndpointKind::Completions => {
                vec![GEMINI_GENERATE_CONTENT_FAMILY.to_string()]
            }
            EndpointKind::ImagesGenerations => vec![GEMINI_GENERATE_CONTENT_FAMILY.to_string()],
            EndpointKind::Embeddings => vec![OPENAI_EMBEDDINGS_FAMILY.to_string()],
            EndpointKind::AudioSpeech => vec![OPENAI_AUDIO_SPEECH_FAMILY.to_string()],
            _ => Vec::new(),
        },
        "bedrock_converse_compatible" => match endpoint_kind {
            EndpointKind::ChatCompletions => vec![BEDROCK_CONVERSE_FAMILY.to_string()],
            _ => Vec::new(),
        },
        "cohere_compatible" => match endpoint_kind {
            EndpointKind::ChatCompletions => vec![COHERE_CHAT_FAMILY.to_string()],
            _ => Vec::new(),
        },
        "gemini_business_compatible" => match endpoint_kind {
            EndpointKind::ImagesGenerations | EndpointKind::ImagesEdits => {
                vec![GEMINI_BUSINESS_IMAGES_FAMILY.to_string()]
            }
            _ => Vec::new(),
        },
        "chataibot_compatible" => match endpoint_kind {
            EndpointKind::ImagesGenerations | EndpointKind::ImagesEdits => {
                vec![CHATAIBOT_IMAGES_FAMILY.to_string()]
            }
            _ => Vec::new(),
        },
        "lumalabs_compatible" => match endpoint_kind {
            EndpointKind::ImagesGenerations => vec![LUMALABS_IMAGES_FAMILY.to_string()],
            EndpointKind::MusicGenerations => vec![LUMALABS_AUDIO_FAMILY.to_string()],
            EndpointKind::VideosGenerations => vec![LUMALABS_VIDEOS_FAMILY.to_string()],
            _ => Vec::new(),
        },
        "gemini_canvas_compatible" => match endpoint_kind {
            EndpointKind::ChatCompletions
            | EndpointKind::Messages
            | EndpointKind::Responses
            | EndpointKind::Completions
            | EndpointKind::AudioSpeech => {
                vec![GEMINI_GENERATE_CONTENT_FAMILY.to_string()]
            }
            EndpointKind::ImagesGenerations | EndpointKind::ImagesEdits => {
                vec![GEMINI_CANVAS_IMAGES_FAMILY.to_string()]
            }
            EndpointKind::MusicGenerations => vec![GEMINI_CANVAS_MUSIC_FAMILY.to_string()],
            EndpointKind::VideosGenerations => vec![GEMINI_CANVAS_VIDEOS_FAMILY.to_string()],
            _ => Vec::new(),
        },
        "gemini_canvas_web_reverse_compatible" | "gemini_canvas_program_web_reverse_compatible" => {
            match endpoint_kind {
                EndpointKind::ChatCompletions
                | EndpointKind::Messages
                | EndpointKind::Responses
                | EndpointKind::Completions
                | EndpointKind::AudioSpeech => {
                    vec![GEMINI_GENERATE_CONTENT_FAMILY.to_string()]
                }
                EndpointKind::ImagesGenerations => {
                    vec![GEMINI_CANVAS_IMAGES_FAMILY.to_string()]
                }
                EndpointKind::MusicGenerations => vec![GEMINI_CANVAS_MUSIC_FAMILY.to_string()],
                EndpointKind::VideosGenerations => vec![GEMINI_CANVAS_VIDEOS_FAMILY.to_string()],
                _ => Vec::new(),
            }
        }
        "gemini_web_reverse_modular_compatible" => match endpoint_kind {
            EndpointKind::ChatCompletions
            | EndpointKind::Messages
            | EndpointKind::Responses
            | EndpointKind::Completions => {
                vec![GEMINI_WEB_CHAT_FAMILY.to_string()]
            }
            EndpointKind::AudioSpeech => vec![GEMINI_GENERATE_CONTENT_FAMILY.to_string()],
            EndpointKind::ImagesGenerations | EndpointKind::ImagesEdits => {
                vec![GEMINI_CANVAS_IMAGES_FAMILY.to_string()]
            }
            EndpointKind::MusicGenerations => vec![GEMINI_CANVAS_MUSIC_FAMILY.to_string()],
            EndpointKind::VideosGenerations => vec![GEMINI_CANVAS_VIDEOS_FAMILY.to_string()],
            _ => Vec::new(),
        },
        "producer_compatible" => match endpoint_kind {
            EndpointKind::ImagesGenerations => vec![PRODUCER_IMAGES_FAMILY.to_string()],
            EndpointKind::MusicGenerations => vec![PRODUCER_MUSIC_FAMILY.to_string()],
            EndpointKind::VideosGenerations => vec![PRODUCER_VIDEOS_FAMILY.to_string()],
            _ => Vec::new(),
        },
        "suno_compatible" => match endpoint_kind {
            EndpointKind::ImagesGenerations => vec![SUNO_IMAGES_FAMILY.to_string()],
            EndpointKind::MusicGenerations => vec![SUNO_MUSIC_FAMILY.to_string()],
            EndpointKind::VideosGenerations => vec![SUNO_VIDEOS_FAMILY.to_string()],
            _ => Vec::new(),
        },
        "udio_compatible" => match endpoint_kind {
            EndpointKind::ImagesGenerations => vec![UDIO_IMAGES_FAMILY.to_string()],
            EndpointKind::MusicGenerations => vec![UDIO_MUSIC_FAMILY.to_string()],
            EndpointKind::VideosGenerations => vec![UDIO_VIDEOS_FAMILY.to_string()],
            _ => Vec::new(),
        },
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
