use serde_json::Value;

use crate::protocol::canonical::{CanonicalRelayRequest, EndpointKind, ProtocolFamily};
use crate::protocol::registry::{
    canonicalize_wire_protocol_family_key, protocol_family_selector_matches,
    ANTHROPIC_MESSAGES_FAMILY, BEDROCK_CONVERSE_FAMILY, CHATAIBOT_IMAGES_FAMILY,
    CHATGPT_WEB_CHAT_FAMILY, COHERE_CHAT_FAMILY, GEMINI_BUSINESS_IMAGES_FAMILY,
    GEMINI_CANVAS_IMAGES_FAMILY, GEMINI_CANVAS_MUSIC_FAMILY, GEMINI_CANVAS_VIDEOS_FAMILY,
    GEMINI_GENERATE_CONTENT_FAMILY, GEMINI_LIVE_FAMILY, GEMINI_WEB_CHAT_FAMILY,
    LUMALABS_AUDIO_FAMILY, LUMALABS_IMAGES_FAMILY, LUMALABS_VIDEOS_FAMILY,
    OPENAI_AUDIO_SPEECH_FAMILY, OPENAI_AUDIO_TRANSCRIPTIONS_FAMILY, OPENAI_CHAT_FAMILY,
    OPENAI_EMBEDDINGS_FAMILY, OPENAI_IMAGES_EDITS_FAMILY, OPENAI_IMAGES_GENERATIONS_FAMILY,
    OPENAI_LEGACY_COMPLETIONS_FAMILY, OPENAI_MUSIC_GENERATIONS_FAMILY, OPENAI_REALTIME_FAMILY,
    OPENAI_RESPONSES_FAMILY, OPENAI_VIDEOS_GENERATIONS_FAMILY, PRODUCER_IMAGES_FAMILY,
    PRODUCER_MUSIC_FAMILY, PRODUCER_VIDEOS_FAMILY, SEARCH_API_FAMILY, SUNO_IMAGES_FAMILY,
    SUNO_MUSIC_FAMILY, SUNO_VIDEOS_FAMILY, UDIO_IMAGES_FAMILY, UDIO_MUSIC_FAMILY,
    UDIO_VIDEOS_FAMILY,
};

use super::candidate::{canonicalize_adapter_name, ProviderAccountPayload, RouteCandidate};

pub const SAME_PROTOCOL_FAMILY_PRIORITY_BONUS: i32 = 5;

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

pub fn resolve_supported_wire_protocol_families_for_model(
    payload: Option<&Value>,
    model_alias: Option<&str>,
    upstream_model: Option<&str>,
    adapter: &str,
    fallback_protocol_family: &str,
) -> Vec<String> {
    let surface_supported =
        surface_supported_wire_protocol_families(adapter, fallback_protocol_family);
    let Some(payload) = payload else {
        return surface_supported;
    };

    let model_candidates = collect_model_candidates(model_alias, upstream_model);
    let model_allowed = read_model_scoped_family_values(
        payload,
        &model_candidates,
        &[
            "protocolFamiliesByModel",
            "protocol_families_by_model",
            "supportedProtocolFamiliesByModel",
            "supported_protocol_families_by_model",
            "allowedProtocolFamiliesByModel",
            "allowed_protocol_families_by_model",
        ],
        &[
            "protocolFamilies",
            "protocol_families",
            "supportedProtocolFamilies",
            "supported_protocol_families",
            "allowedProtocolFamilies",
            "allowed_protocol_families",
            "families",
        ],
    );
    let global_allowed = read_family_values_from_object(
        payload,
        &[
            "protocolFamilies",
            "protocol_families",
            "supportedProtocolFamilies",
            "supported_protocol_families",
            "allowedProtocolFamilies",
            "allowed_protocol_families",
            "protocolFamily",
            "protocol_family",
        ],
    );
    let mut allowed = if !model_allowed.is_empty() {
        model_allowed
    } else if !global_allowed.is_empty() {
        global_allowed
    } else {
        surface_supported.clone()
    };
    if allowed.is_empty() {
        allowed = surface_supported.clone();
    }

    let model_blocked = read_model_scoped_family_values(
        payload,
        &model_candidates,
        &[
            "excludedProtocolFamiliesByModel",
            "excluded_protocol_families_by_model",
            "blockedProtocolFamiliesByModel",
            "blocked_protocol_families_by_model",
        ],
        &[
            "excludedProtocolFamilies",
            "excluded_protocol_families",
            "blockedProtocolFamilies",
            "blocked_protocol_families",
            "families",
        ],
    );
    let global_blocked = read_family_values_from_object(
        payload,
        &[
            "excludedProtocolFamilies",
            "excluded_protocol_families",
            "blockedProtocolFamilies",
            "blocked_protocol_families",
        ],
    );

    let mut resolved = Vec::new();
    for family in allowed {
        let canonical = canonicalize_wire_protocol_family_key(&family);
        if !surface_supported.iter().any(|value| value == &canonical) {
            continue;
        }
        if global_blocked
            .iter()
            .chain(model_blocked.iter())
            .any(|blocked| blocked == &canonical)
        {
            continue;
        }
        push_unique(&mut resolved, canonical);
    }
    resolved
}

pub fn finalize_candidate_protocol_family(
    candidate: &mut RouteCandidate,
    req: &CanonicalRelayRequest,
) -> Option<bool> {
    let supported = if candidate.supported_protocol_families.is_empty() {
        surface_supported_wire_protocol_families(&candidate.adapter, &candidate.protocol_family)
    } else {
        candidate
            .supported_protocol_families
            .iter()
            .map(|value| canonicalize_wire_protocol_family_key(value))
            .collect()
    };
    let request_compatible = request_compatible_wire_protocol_families(
        &candidate.payload,
        req.endpoint_kind,
        &candidate.protocol_family,
    );
    let compatible = request_compatible
        .into_iter()
        .filter(|family| supported.iter().any(|allowed| allowed == family))
        .collect::<Vec<_>>();
    if compatible.is_empty() {
        return None;
    }

    let requested = requested_wire_protocol_family(req);
    let default_family = canonicalize_wire_protocol_family_key(&candidate.protocol_family);
    let same_family = requested
        .as_ref()
        .is_some_and(|family| compatible.iter().any(|value| value == family));

    let selected = if let Some(requested) = requested {
        if compatible.iter().any(|value| value == &requested) {
            requested
        } else if compatible.iter().any(|value| value == &default_family) {
            default_family
        } else {
            compatible[0].clone()
        }
    } else if compatible.iter().any(|value| value == &default_family) {
        default_family
    } else {
        compatible[0].clone()
    };

    candidate.protocol_family = selected;
    if same_family {
        candidate.priority = candidate
            .priority
            .saturating_add(SAME_PROTOCOL_FAMILY_PRIORITY_BONUS);
    }
    Some(same_family)
}

pub fn route_policy_family_matches_surface(
    allowed_family: &str,
    adapter: &str,
    protocol_family: &str,
) -> bool {
    surface_supported_wire_protocol_families(adapter, protocol_family)
        .iter()
        .any(|value| protocol_family_selector_matches(allowed_family, value))
}

pub fn route_policy_family_matches_candidate(
    allowed_family: &str,
    candidate_protocol_family: &str,
) -> bool {
    protocol_family_selector_matches(allowed_family, candidate_protocol_family)
}

fn request_compatible_wire_protocol_families(
    payload: &ProviderAccountPayload,
    endpoint_kind: EndpointKind,
    fallback_protocol_family: &str,
) -> Vec<String> {
    match payload.canonical_adapter() {
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
        "gemini_canvas_compatible"
        | "gemini_canvas_web_reverse_compatible"
        | "gemini_canvas_program_web_reverse_compatible" => match endpoint_kind {
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

fn collect_model_candidates(
    model_alias: Option<&str>,
    upstream_model: Option<&str>,
) -> Vec<String> {
    let mut values = Vec::new();
    if let Some(value) = model_alias.and_then(normalize_model_hint) {
        push_unique(&mut values, value);
    }
    if let Some(value) = upstream_model.and_then(normalize_model_hint) {
        push_unique(&mut values, value);
    }
    values
}

fn normalize_model_hint(value: &str) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then_some(trimmed.to_ascii_lowercase())
}

fn model_key_matches(pattern: &str, candidates: &[String]) -> bool {
    let normalized = canonicalize_model_pattern(pattern);
    if normalized.is_empty() {
        return false;
    }
    candidates.iter().any(|candidate| {
        if let Some(prefix) = normalized.strip_suffix('*') {
            candidate.starts_with(prefix)
        } else {
            candidate == &normalized
        }
    })
}

fn canonicalize_model_pattern(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

fn read_family_values_from_object(payload: &Value, keys: &[&str]) -> Vec<String> {
    let Some(object) = payload.as_object() else {
        return Vec::new();
    };
    let mut values = Vec::new();
    for key in keys {
        if let Some(value) = object.get(*key) {
            collect_family_values(value, &mut values);
        }
    }
    values
}

fn read_model_scoped_family_values(
    payload: &Value,
    model_candidates: &[String],
    top_level_keys: &[&str],
    nested_keys: &[&str],
) -> Vec<String> {
    let Some(object) = payload.as_object() else {
        return Vec::new();
    };
    let mut values = Vec::new();
    for key in top_level_keys {
        let Some(Value::Object(model_map)) = object.get(*key) else {
            continue;
        };
        for (model_key, value) in model_map {
            if !model_key_matches(model_key, model_candidates) {
                continue;
            }
            match value {
                Value::Object(nested) => {
                    for nested_key in nested_keys {
                        if let Some(nested_value) = nested.get(*nested_key) {
                            collect_family_values(nested_value, &mut values);
                        }
                    }
                }
                _ => collect_family_values(value, &mut values),
            }
        }
    }
    values
}

fn collect_family_values(value: &Value, output: &mut Vec<String>) {
    match value {
        Value::String(text) => {
            for item in text
                .split([',', '\n'])
                .map(str::trim)
                .filter(|item| !item.is_empty())
            {
                push_unique(output, canonicalize_wire_protocol_family_key(item));
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_family_values(item, output);
            }
        }
        _ => {}
    }
}

fn push_unique(values: &mut Vec<String>, value: String) {
    if !values.iter().any(|existing| existing == &value) {
        values.push(value);
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::protocol::canonical::{
        CanonicalMessage, CanonicalRelayRequest, ContentPart, MessageRole,
    };
    use crate::routing::candidate::{ProviderExecutionMode, RouteCandidate};

    use super::*;

    fn make_request(
        protocol_family: ProtocolFamily,
        endpoint_kind: EndpointKind,
    ) -> CanonicalRelayRequest {
        CanonicalRelayRequest {
            protocol_family,
            endpoint_kind,
            requested_model: Some("gpt-4o".to_string()),
            stream: false,
            messages: vec![CanonicalMessage {
                role: MessageRole::User,
                content: vec![ContentPart::Text {
                    text: "hello".to_string(),
                }],
                name: None,
                tool_call_id: None,
                tool_calls: vec![],
            }],
            tools: vec![],
            tool_choice: None,
            reasoning: None,
            metadata: None,
            raw_body: json!({}),
            previous_response_id: None,
            explicit_session_key: None,
            extra: Default::default(),
        }
    }

    fn make_candidate(protocol_family: &str) -> RouteCandidate {
        RouteCandidate {
            provider_account_id: "provider-1".to_string(),
            provider_credential_id: Some("cred-1".to_string()),
            label: "provider-1".to_string(),
            payload: ProviderAccountPayload {
                adapter: "openai_compatible".to_string(),
                base_url: "https://api.example.com".to_string(),
                api_key: "sk-test".to_string(),
                credential_id: None,
                expires_at: None,
                runtime_state_object_key: None,
                account_name: None,
                execution_mode: None,
                endpoint_execution_modes: None,
                default_model: None,
                headers: Default::default(),
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
            },
            protocol_family: protocol_family.to_string(),
            protocol_profile: "openai".to_string(),
            supported_protocol_families: vec![
                OPENAI_CHAT_FAMILY.to_string(),
                OPENAI_RESPONSES_FAMILY.to_string(),
            ],
            adapter: "openai_compatible".to_string(),
            model_alias: Some("gpt-4o".to_string()),
            upstream_model: Some("gpt-4o".to_string()),
            resolved_execution_mode: ProviderExecutionMode::DirectHttp,
            priority: 10,
            weight: 1,
            failure_count: 0,
            cooldown_until: None,
            routing_score: None,
            routing_health_weight: None,
            routing_capacity_weight: None,
            routing_degraded: None,
            routing_breaker_open: None,
            routing_degradation_reasons: Vec::new(),
        }
    }

    #[test]
    fn payload_model_family_matrix_overrides_global_family_list() {
        let payload = json!({
            "supportedProtocolFamilies": ["openai_chat", "openai_responses"],
            "protocolFamiliesByModel": {
                "gpt-4o": ["openai_responses"]
            }
        });
        let resolved = resolve_supported_wire_protocol_families_for_model(
            Some(&payload),
            Some("gpt-4o"),
            Some("gpt-4o"),
            "openai_compatible",
            "openai",
        );
        assert_eq!(resolved, vec![OPENAI_RESPONSES_FAMILY.to_string()]);
    }

    #[test]
    fn finalize_candidate_prefers_same_family_when_supported() {
        let req = make_request(ProtocolFamily::OpenAi, EndpointKind::Responses);
        let mut candidate = make_candidate("openai");
        let same_family = finalize_candidate_protocol_family(&mut candidate, &req);
        assert_eq!(same_family, Some(true));
        assert_eq!(candidate.protocol_family, OPENAI_RESPONSES_FAMILY);
        assert_eq!(candidate.priority, 10 + SAME_PROTOCOL_FAMILY_PRIORITY_BONUS);
    }

    #[test]
    fn finalize_candidate_filters_out_surface_with_no_matching_supported_family() {
        let req = make_request(ProtocolFamily::OpenAi, EndpointKind::ChatCompletions);
        let mut candidate = make_candidate("openai");
        candidate.supported_protocol_families = vec![ANTHROPIC_MESSAGES_FAMILY.to_string()];
        let same_family = finalize_candidate_protocol_family(&mut candidate, &req);
        assert_eq!(same_family, None);
    }

    #[test]
    fn route_policy_generic_search_family_matches_explicit_search_surface() {
        assert!(route_policy_family_matches_surface(
            "search_api",
            "search_api_compatible",
            "linkup_search",
        ));
        assert!(route_policy_family_matches_candidate(
            "search_api",
            "tavily_search",
        ));
    }

    #[test]
    fn requested_wire_protocol_family_maps_openai_modality_endpoints() {
        let req = make_request(ProtocolFamily::OpenAi, EndpointKind::Embeddings);
        assert_eq!(
            requested_wire_protocol_family(&req).as_deref(),
            Some(OPENAI_EMBEDDINGS_FAMILY)
        );

        let req = make_request(ProtocolFamily::OpenAi, EndpointKind::AudioSpeech);
        assert_eq!(
            requested_wire_protocol_family(&req).as_deref(),
            Some(OPENAI_AUDIO_SPEECH_FAMILY)
        );
    }

    #[test]
    fn finalize_candidate_selects_special_media_family_for_native_adapter() {
        let req = make_request(ProtocolFamily::OpenAi, EndpointKind::MusicGenerations);
        let mut candidate = make_candidate(PRODUCER_MUSIC_FAMILY);
        candidate.adapter = "producer_compatible".to_string();
        candidate.payload.adapter = "producer_compatible".to_string();
        candidate.protocol_family = PRODUCER_MUSIC_FAMILY.to_string();
        candidate.supported_protocol_families = vec![
            PRODUCER_MUSIC_FAMILY.to_string(),
            PRODUCER_VIDEOS_FAMILY.to_string(),
        ];

        let same_family = finalize_candidate_protocol_family(&mut candidate, &req);
        assert_eq!(same_family, Some(false));
        assert_eq!(candidate.protocol_family, PRODUCER_MUSIC_FAMILY);
    }

    #[test]
    fn lumalabs_surface_supports_image_video_and_audio_families() {
        let families =
            surface_supported_wire_protocol_families("lumalabs_compatible", LUMALABS_IMAGES_FAMILY);
        assert!(families.contains(&LUMALABS_IMAGES_FAMILY.to_string()));
        assert!(families.contains(&LUMALABS_AUDIO_FAMILY.to_string()));
        assert!(families.contains(&LUMALABS_VIDEOS_FAMILY.to_string()));
    }

    #[test]
    fn suno_surface_supports_image_video_and_audio_families() {
        let families =
            surface_supported_wire_protocol_families("suno_compatible", SUNO_MUSIC_FAMILY);
        assert!(families.contains(&SUNO_IMAGES_FAMILY.to_string()));
        assert!(families.contains(&SUNO_MUSIC_FAMILY.to_string()));
        assert!(families.contains(&SUNO_VIDEOS_FAMILY.to_string()));
    }

    #[test]
    fn producer_surface_supports_image_video_and_audio_families() {
        let families =
            surface_supported_wire_protocol_families("producer_compatible", PRODUCER_MUSIC_FAMILY);
        assert!(families.contains(&PRODUCER_IMAGES_FAMILY.to_string()));
        assert!(families.contains(&PRODUCER_MUSIC_FAMILY.to_string()));
        assert!(families.contains(&PRODUCER_VIDEOS_FAMILY.to_string()));
    }

    #[test]
    fn gemini_api_surface_supports_text_tts_and_media_families() {
        let families = surface_supported_wire_protocol_families(
            "gemini_api_modular_compatible",
            GEMINI_GENERATE_CONTENT_FAMILY,
        );
        assert!(families.contains(&GEMINI_GENERATE_CONTENT_FAMILY.to_string()));
        assert!(families.contains(&GEMINI_CANVAS_IMAGES_FAMILY.to_string()));
        assert!(families.contains(&GEMINI_CANVAS_MUSIC_FAMILY.to_string()));
        assert!(families.contains(&GEMINI_CANVAS_VIDEOS_FAMILY.to_string()));
    }

    #[test]
    fn udio_surface_supports_image_video_and_audio_families() {
        let families =
            surface_supported_wire_protocol_families("udio_compatible", UDIO_MUSIC_FAMILY);
        assert!(families.contains(&UDIO_IMAGES_FAMILY.to_string()));
        assert!(families.contains(&UDIO_MUSIC_FAMILY.to_string()));
        assert!(families.contains(&UDIO_VIDEOS_FAMILY.to_string()));
    }

    #[test]
    fn finalize_candidate_selects_lumalabs_video_family_for_video_requests() {
        let req = make_request(ProtocolFamily::OpenAi, EndpointKind::VideosGenerations);
        let mut candidate = make_candidate(LUMALABS_IMAGES_FAMILY);
        candidate.adapter = "lumalabs_compatible".to_string();
        candidate.payload.adapter = "lumalabs_compatible".to_string();
        candidate.protocol_family = LUMALABS_IMAGES_FAMILY.to_string();
        candidate.supported_protocol_families = vec![
            LUMALABS_IMAGES_FAMILY.to_string(),
            LUMALABS_AUDIO_FAMILY.to_string(),
            LUMALABS_VIDEOS_FAMILY.to_string(),
        ];

        let same_family = finalize_candidate_protocol_family(&mut candidate, &req);
        assert_eq!(same_family, Some(false));
        assert_eq!(candidate.protocol_family, LUMALABS_VIDEOS_FAMILY);
    }

    #[test]
    fn finalize_candidate_selects_suno_video_family_for_video_requests() {
        let req = make_request(ProtocolFamily::OpenAi, EndpointKind::VideosGenerations);
        let mut candidate = make_candidate(SUNO_MUSIC_FAMILY);
        candidate.adapter = "suno_compatible".to_string();
        candidate.payload.adapter = "suno_compatible".to_string();
        candidate.protocol_family = SUNO_MUSIC_FAMILY.to_string();
        candidate.supported_protocol_families = vec![
            SUNO_IMAGES_FAMILY.to_string(),
            SUNO_MUSIC_FAMILY.to_string(),
            SUNO_VIDEOS_FAMILY.to_string(),
        ];

        let same_family = finalize_candidate_protocol_family(&mut candidate, &req);
        assert_eq!(same_family, Some(false));
        assert_eq!(candidate.protocol_family, SUNO_VIDEOS_FAMILY);
    }

    #[test]
    fn finalize_candidate_selects_producer_image_family_for_image_requests() {
        let req = make_request(ProtocolFamily::OpenAi, EndpointKind::ImagesGenerations);
        let mut candidate = make_candidate(PRODUCER_MUSIC_FAMILY);
        candidate.adapter = "producer_compatible".to_string();
        candidate.payload.adapter = "producer_compatible".to_string();
        candidate.protocol_family = PRODUCER_MUSIC_FAMILY.to_string();
        candidate.supported_protocol_families = vec![
            PRODUCER_IMAGES_FAMILY.to_string(),
            PRODUCER_MUSIC_FAMILY.to_string(),
            PRODUCER_VIDEOS_FAMILY.to_string(),
        ];

        let same_family = finalize_candidate_protocol_family(&mut candidate, &req);
        assert_eq!(same_family, Some(false));
        assert_eq!(candidate.protocol_family, PRODUCER_IMAGES_FAMILY);
    }

    #[test]
    fn finalize_candidate_selects_gemini_canvas_image_family_for_image_edit_requests() {
        let req = make_request(ProtocolFamily::OpenAi, EndpointKind::ImagesEdits);
        let mut candidate = make_candidate(GEMINI_CANVAS_MUSIC_FAMILY);
        candidate.adapter = "gemini_canvas_compatible".to_string();
        candidate.payload.adapter = "gemini_canvas_compatible".to_string();
        candidate.protocol_family = GEMINI_CANVAS_MUSIC_FAMILY.to_string();
        candidate.supported_protocol_families = vec![
            GEMINI_CANVAS_IMAGES_FAMILY.to_string(),
            GEMINI_CANVAS_MUSIC_FAMILY.to_string(),
            GEMINI_CANVAS_VIDEOS_FAMILY.to_string(),
        ];

        let same_family = finalize_candidate_protocol_family(&mut candidate, &req);
        assert_eq!(same_family, Some(false));
        assert_eq!(candidate.protocol_family, GEMINI_CANVAS_IMAGES_FAMILY);
    }

    #[test]
    fn finalize_candidate_selects_udio_video_family_for_video_requests() {
        let req = make_request(ProtocolFamily::OpenAi, EndpointKind::VideosGenerations);
        let mut candidate = make_candidate(UDIO_MUSIC_FAMILY);
        candidate.adapter = "udio_compatible".to_string();
        candidate.payload.adapter = "udio_compatible".to_string();
        candidate.protocol_family = UDIO_MUSIC_FAMILY.to_string();
        candidate.supported_protocol_families = vec![
            UDIO_IMAGES_FAMILY.to_string(),
            UDIO_MUSIC_FAMILY.to_string(),
            UDIO_VIDEOS_FAMILY.to_string(),
        ];

        let same_family = finalize_candidate_protocol_family(&mut candidate, &req);
        assert_eq!(same_family, Some(false));
        assert_eq!(candidate.protocol_family, UDIO_VIDEOS_FAMILY);
    }

    #[test]
    fn openai_compatible_completions_falls_back_to_chat_family_without_legacy_path() {
        let payload = ProviderAccountPayload {
            chat_completions_path: Some("/v2/chat/completions".to_string()),
            ..make_candidate("openai").payload
        };
        let families = request_compatible_wire_protocol_families(
            &payload,
            EndpointKind::Completions,
            "openai",
        );
        assert_eq!(families, vec![OPENAI_CHAT_FAMILY.to_string()]);
    }

    #[test]
    fn openai_compatible_completions_uses_legacy_family_when_path_is_present() {
        let payload = ProviderAccountPayload {
            chat_completions_path: Some("/v2/chat/completions".to_string()),
            completions_path: Some("/v1/completions".to_string()),
            ..make_candidate("openai").payload
        };
        let families = request_compatible_wire_protocol_families(
            &payload,
            EndpointKind::Completions,
            "openai",
        );
        assert_eq!(families, vec![OPENAI_LEGACY_COMPLETIONS_FAMILY.to_string()]);
    }

    #[test]
    fn openai_compatible_responses_fall_back_to_chat_family_when_only_chat_path_is_present() {
        let payload = ProviderAccountPayload {
            chat_completions_path: Some("/v1/chat/completions".to_string()),
            ..make_candidate("openai").payload
        };
        let families =
            request_compatible_wire_protocol_families(&payload, EndpointKind::Responses, "openai");
        assert_eq!(families, vec![OPENAI_CHAT_FAMILY.to_string()]);
    }

    #[test]
    fn anthropic_compatible_supports_cross_family_text_ingress() {
        let payload = ProviderAccountPayload {
            adapter: "anthropic_compatible".to_string(),
            ..make_candidate("anthropic").payload
        };
        for endpoint_kind in [
            EndpointKind::ChatCompletions,
            EndpointKind::Completions,
            EndpointKind::Messages,
            EndpointKind::Responses,
        ] {
            let families =
                request_compatible_wire_protocol_families(&payload, endpoint_kind, "anthropic");
            assert_eq!(families, vec![ANTHROPIC_MESSAGES_FAMILY.to_string()]);
        }
    }

    #[test]
    fn freebuff_compatible_supports_cross_family_text_ingress() {
        let payload = ProviderAccountPayload {
            adapter: "freebuff_compatible".to_string(),
            ..make_candidate("freebuff").payload
        };
        for endpoint_kind in [
            EndpointKind::ChatCompletions,
            EndpointKind::Completions,
            EndpointKind::Messages,
            EndpointKind::Responses,
        ] {
            let families =
                request_compatible_wire_protocol_families(&payload, endpoint_kind, "freebuff");
            assert_eq!(families, vec![OPENAI_CHAT_FAMILY.to_string()]);
        }
    }

    #[test]
    fn accio_compatible_supports_cross_family_text_ingress_via_responses_bridge() {
        let payload = ProviderAccountPayload {
            adapter: "accio_compatible".to_string(),
            ..make_candidate("openai_responses").payload
        };
        for endpoint_kind in [
            EndpointKind::ChatCompletions,
            EndpointKind::Completions,
            EndpointKind::Messages,
            EndpointKind::Responses,
        ] {
            let families = request_compatible_wire_protocol_families(
                &payload,
                endpoint_kind,
                OPENAI_RESPONSES_FAMILY,
            );
            assert_eq!(families, vec![OPENAI_RESPONSES_FAMILY.to_string()]);
        }
    }

    #[test]
    fn gemini_canvas_web_reverse_modular_supports_text_and_tts_ingress() {
        let payload = ProviderAccountPayload {
            adapter: "gemini_canvas_web_reverse_compatible".to_string(),
            ..make_candidate("gemini_canvas").payload
        };
        for endpoint_kind in [
            EndpointKind::ChatCompletions,
            EndpointKind::Completions,
            EndpointKind::Messages,
            EndpointKind::Responses,
            EndpointKind::AudioSpeech,
        ] {
            let families =
                request_compatible_wire_protocol_families(&payload, endpoint_kind, "gemini_canvas");
            assert_eq!(families, vec![GEMINI_GENERATE_CONTENT_FAMILY.to_string()]);
        }
    }

    #[test]
    fn gemini_canvas_program_web_reverse_modular_supports_text_tts_and_media_ingress() {
        let payload = ProviderAccountPayload {
            adapter: "gemini_canvas_program_web_reverse_compatible".to_string(),
            ..make_candidate("gemini_canvas").payload
        };

        for endpoint_kind in [
            EndpointKind::ChatCompletions,
            EndpointKind::Completions,
            EndpointKind::Messages,
            EndpointKind::Responses,
            EndpointKind::AudioSpeech,
        ] {
            let families =
                request_compatible_wire_protocol_families(&payload, endpoint_kind, "gemini_canvas");
            assert_eq!(families, vec![GEMINI_GENERATE_CONTENT_FAMILY.to_string()]);
        }

        assert_eq!(
            request_compatible_wire_protocol_families(
                &payload,
                EndpointKind::ImagesGenerations,
                "gemini_canvas"
            ),
            vec![GEMINI_CANVAS_IMAGES_FAMILY.to_string()]
        );
        assert_eq!(
            request_compatible_wire_protocol_families(
                &payload,
                EndpointKind::ImagesEdits,
                "gemini_canvas"
            ),
            vec![GEMINI_CANVAS_IMAGES_FAMILY.to_string()]
        );
        assert_eq!(
            request_compatible_wire_protocol_families(
                &payload,
                EndpointKind::MusicGenerations,
                "gemini_canvas"
            ),
            vec![GEMINI_CANVAS_MUSIC_FAMILY.to_string()]
        );
        assert_eq!(
            request_compatible_wire_protocol_families(
                &payload,
                EndpointKind::VideosGenerations,
                "gemini_canvas"
            ),
            vec![GEMINI_CANVAS_VIDEOS_FAMILY.to_string()]
        );
    }

    #[test]
    fn gemini_api_modular_supports_cross_family_text_ingress() {
        let payload = ProviderAccountPayload {
            adapter: "gemini_api_modular_compatible".to_string(),
            ..make_candidate("gemini_generate_content").payload
        };
        for endpoint_kind in [
            EndpointKind::ChatCompletions,
            EndpointKind::Completions,
            EndpointKind::Messages,
            EndpointKind::Responses,
        ] {
            let families = request_compatible_wire_protocol_families(
                &payload,
                endpoint_kind,
                "gemini_generate_content",
            );
            assert_eq!(families, vec![GEMINI_GENERATE_CONTENT_FAMILY.to_string()]);
        }
    }

    #[test]
    fn gemini_api_modular_supports_media_and_tts_endpoint_families() {
        let payload = ProviderAccountPayload {
            adapter: "gemini_api_modular_compatible".to_string(),
            ..make_candidate("gemini_generate_content").payload
        };
        assert_eq!(
            request_compatible_wire_protocol_families(
                &payload,
                EndpointKind::AudioSpeech,
                "gemini_generate_content",
            ),
            vec![GEMINI_GENERATE_CONTENT_FAMILY.to_string()]
        );
        assert_eq!(
            request_compatible_wire_protocol_families(
                &payload,
                EndpointKind::ImagesGenerations,
                "gemini_generate_content",
            ),
            vec![GEMINI_CANVAS_IMAGES_FAMILY.to_string()]
        );
        assert_eq!(
            request_compatible_wire_protocol_families(
                &payload,
                EndpointKind::MusicGenerations,
                "gemini_generate_content",
            ),
            vec![GEMINI_CANVAS_MUSIC_FAMILY.to_string()]
        );
        assert_eq!(
            request_compatible_wire_protocol_families(
                &payload,
                EndpointKind::VideosGenerations,
                "gemini_generate_content",
            ),
            vec![GEMINI_CANVAS_VIDEOS_FAMILY.to_string()]
        );
    }

    #[test]
    fn aistudio_web_reverse_supports_cross_family_text_ingress() {
        let payload = ProviderAccountPayload {
            adapter: "aistudio_web_reverse_compatible".to_string(),
            ..make_candidate("gemini_generate_content").payload
        };
        for endpoint_kind in [
            EndpointKind::ChatCompletions,
            EndpointKind::Completions,
            EndpointKind::Messages,
            EndpointKind::Responses,
        ] {
            let families = request_compatible_wire_protocol_families(
                &payload,
                endpoint_kind,
                "gemini_generate_content",
            );
            assert_eq!(families, vec![GEMINI_GENERATE_CONTENT_FAMILY.to_string()]);
        }
    }

    #[test]
    fn aistudio_web_reverse_supports_embeddings_ingress() {
        let payload = ProviderAccountPayload {
            adapter: "aistudio_web_reverse_compatible".to_string(),
            ..make_candidate("gemini_generate_content").payload
        };
        let families = request_compatible_wire_protocol_families(
            &payload,
            EndpointKind::Embeddings,
            "gemini_generate_content",
        );
        assert_eq!(families, vec![OPENAI_EMBEDDINGS_FAMILY.to_string()]);
    }

    #[test]
    fn aistudio_web_reverse_supports_audio_speech_ingress() {
        let payload = ProviderAccountPayload {
            adapter: "aistudio_web_reverse_compatible".to_string(),
            ..make_candidate("gemini_generate_content").payload
        };
        let families = request_compatible_wire_protocol_families(
            &payload,
            EndpointKind::AudioSpeech,
            "gemini_generate_content",
        );
        assert_eq!(families, vec![OPENAI_AUDIO_SPEECH_FAMILY.to_string()]);
    }

    #[test]
    fn aistudio_web_reverse_supports_image_generation_ingress() {
        let payload = ProviderAccountPayload {
            adapter: "aistudio_web_reverse_compatible".to_string(),
            ..make_candidate("gemini_generate_content").payload
        };
        let families = request_compatible_wire_protocol_families(
            &payload,
            EndpointKind::ImagesGenerations,
            "gemini_generate_content",
        );
        assert_eq!(families, vec![GEMINI_GENERATE_CONTENT_FAMILY.to_string()]);
    }

    #[test]
    fn chatgpt_web_reverse_supports_cross_family_text_ingress() {
        let payload = ProviderAccountPayload {
            adapter: "chatgpt_web_reverse_compatible".to_string(),
            ..make_candidate("openai_chat").payload
        };
        for endpoint_kind in [
            EndpointKind::ChatCompletions,
            EndpointKind::Completions,
            EndpointKind::Messages,
            EndpointKind::Responses,
        ] {
            let families = request_compatible_wire_protocol_families(
                &payload,
                endpoint_kind,
                OPENAI_CHAT_FAMILY,
            );
            assert_eq!(families, vec![CHATGPT_WEB_CHAT_FAMILY.to_string()]);
        }
    }

    #[test]
    fn gemini_web_reverse_modular_supports_legacy_mixed_lane_tts_and_media_ingress() {
        let payload = ProviderAccountPayload {
            adapter: "gemini_web_reverse_modular_compatible".to_string(),
            ..make_candidate("gemini_web_chat").payload
        };

        assert_eq!(
            request_compatible_wire_protocol_families(
                &payload,
                EndpointKind::AudioSpeech,
                "gemini_web_chat",
            ),
            vec![GEMINI_GENERATE_CONTENT_FAMILY.to_string()]
        );
        assert_eq!(
            request_compatible_wire_protocol_families(
                &payload,
                EndpointKind::ImagesGenerations,
                "gemini_web_chat",
            ),
            vec![GEMINI_CANVAS_IMAGES_FAMILY.to_string()]
        );
        assert_eq!(
            request_compatible_wire_protocol_families(
                &payload,
                EndpointKind::ImagesEdits,
                "gemini_web_chat",
            ),
            vec![GEMINI_CANVAS_IMAGES_FAMILY.to_string()]
        );
        assert_eq!(
            request_compatible_wire_protocol_families(
                &payload,
                EndpointKind::MusicGenerations,
                "gemini_web_chat",
            ),
            vec![GEMINI_CANVAS_MUSIC_FAMILY.to_string()]
        );
        assert_eq!(
            request_compatible_wire_protocol_families(
                &payload,
                EndpointKind::VideosGenerations,
                "gemini_web_chat",
            ),
            vec![GEMINI_CANVAS_VIDEOS_FAMILY.to_string()]
        );
    }
}
