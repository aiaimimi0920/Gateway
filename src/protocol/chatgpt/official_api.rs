use crate::protocol::canonical::EndpointKind;
use crate::protocol::openai;
use crate::protocol::registry::canonicalize_protocol_profile_key;
use crate::routing::candidate::ProviderAccountPayload;
use serde_json::Value;

pub const CHATGPT_OFFICIAL_API_PROFILE: &str = "chatgpt_official_api";
pub const CHATGPT_OFFICIAL_API_PRESET_ID: &str = "openai";
pub const CHATGPT_OFFICIAL_API_CATALOG_KEY: &str = "openai-platform";
pub const CHATGPT_CODEX_BACKEND_PRESET_ID: &str = "codex";
pub const CHATGPT_CODEX_BACKEND_PROFILE: &str = "chatgpt_codex_backend";
pub const CHATGPT_CODEX_BACKEND_PATH_SEGMENT: &str = "chatgpt.com/backend-api/codex";

pub fn is_chatgpt_official_api_profile(value: &str) -> bool {
    canonicalize_protocol_profile_key(value) == CHATGPT_OFFICIAL_API_PROFILE
}

pub fn is_chatgpt_official_api_preset_id(value: &str) -> bool {
    value.trim() == CHATGPT_OFFICIAL_API_PRESET_ID
}

pub fn is_chatgpt_official_api_catalog_key(value: &str) -> bool {
    value.trim() == CHATGPT_OFFICIAL_API_CATALOG_KEY
}

pub fn is_chatgpt_official_api_base_url(base_url: &str) -> bool {
    base_url
        .trim_end_matches('/')
        .to_ascii_lowercase()
        .contains("api.openai.com")
}

pub fn is_chatgpt_codex_backend_payload(payload: &ProviderAccountPayload) -> bool {
    if !is_chatgpt_codex_backend_base_url(&payload.base_url) {
        return false;
    }

    payload
        .headers
        .get("Originator")
        .or_else(|| payload.headers.get("originator"))
        .is_some_and(|value| has_chatgpt_codex_originator(value))
}

pub fn is_chatgpt_codex_backend_base_url(base_url: &str) -> bool {
    base_url
        .trim_end_matches('/')
        .to_ascii_lowercase()
        .contains(CHATGPT_CODEX_BACKEND_PATH_SEGMENT)
}

pub fn is_chatgpt_codex_backend_value(payload: &Value) -> bool {
    let base_url = payload
        .get("baseUrl")
        .or_else(|| payload.get("base_url"))
        .and_then(Value::as_str);
    if !base_url.is_some_and(is_chatgpt_codex_backend_base_url) {
        return false;
    }

    payload
        .get("headers")
        .and_then(Value::as_object)
        .and_then(|headers| {
            headers
                .get("Originator")
                .or_else(|| headers.get("originator"))
                .and_then(Value::as_str)
        })
        .is_some_and(has_chatgpt_codex_originator)
}

pub fn provider_line_name(payload: &ProviderAccountPayload) -> &'static str {
    if is_chatgpt_codex_backend_payload(payload) {
        CHATGPT_CODEX_BACKEND_PROFILE
    } else {
        CHATGPT_OFFICIAL_API_PROFILE
    }
}

fn has_chatgpt_codex_originator(value: &str) -> bool {
    value.to_ascii_lowercase().contains("codex")
}

pub fn supports_endpoint(endpoint_kind: EndpointKind) -> bool {
    matches!(
        endpoint_kind,
        EndpointKind::ChatCompletions
            | EndpointKind::Responses
            | EndpointKind::Completions
            | EndpointKind::Embeddings
            | EndpointKind::AudioTranscriptions
            | EndpointKind::AudioSpeech
            | EndpointKind::ImagesGenerations
            | EndpointKind::ImagesEdits
            | EndpointKind::MusicGenerations
            | EndpointKind::VideosGenerations
    )
}

pub use openai::{
    build_chat_completions_delta, build_chat_completions_stop, build_chat_completions_success,
    build_legacy_completions_delta, build_legacy_completions_stop,
    build_legacy_completions_success, normalize_audio_speech, normalize_audio_transcriptions,
    normalize_chat_completions, normalize_embeddings, normalize_legacy_completions, pack_openai,
    translate_openai_chat_sse_to_legacy_completions, unpack_openai_response,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::routing::candidate::ProviderExecutionMode;
    use std::collections::HashMap;

    fn make_payload(base_url: &str, originator: Option<&str>) -> ProviderAccountPayload {
        let mut headers = HashMap::new();
        if let Some(originator) = originator {
            headers.insert("Originator".to_string(), originator.to_string());
        }

        ProviderAccountPayload {
            adapter: "openai_compatible".to_string(),
            base_url: base_url.to_string(),
            api_key: "tok".to_string(),
            credential_id: None,
            expires_at: None,
            runtime_state_object_key: None,
            account_name: None,
            execution_mode: Some(ProviderExecutionMode::DirectHttp),
            endpoint_execution_modes: None,
            default_model: None,
            headers,
            auth_mode: None,
            anthropic_version: None,
            beta_headers: None,
            auth_header_name: None,
            auth_token: None,
            responses_path: Some("/responses".to_string()),
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
    fn recognizes_openai_profile_as_chatgpt_official_api_line() {
        assert!(is_chatgpt_official_api_profile("openai"));
        assert!(is_chatgpt_official_api_profile("chatgpt_official_api"));
        assert!(is_chatgpt_official_api_preset_id("openai"));
        assert!(is_chatgpt_official_api_catalog_key("openai-platform"));
        assert!(is_chatgpt_official_api_base_url(
            "https://api.openai.com/v1"
        ));
        assert!(!is_chatgpt_official_api_profile(
            "openai_compatible_generic"
        ));
    }

    #[test]
    fn codex_backend_requires_codex_originator() {
        assert!(is_chatgpt_codex_backend_payload(&make_payload(
            "https://chatgpt.com/backend-api/codex",
            Some("codex_cli_rs")
        )));
        assert!(!is_chatgpt_codex_backend_payload(&make_payload(
            "https://chatgpt.com/backend-api/codex",
            Some("generic-client")
        )));
        assert!(!is_chatgpt_codex_backend_payload(&make_payload(
            "https://api.openai.com/v1",
            Some("codex_cli_rs")
        )));
    }

    #[test]
    fn codex_backend_value_requires_codex_originator_header() {
        let payload = serde_json::json!({
            "baseUrl": "https://chatgpt.com/backend-api/codex",
            "headers": {
                "Originator": "codex_cli_rs"
            }
        });
        assert!(is_chatgpt_codex_backend_value(&payload));

        let missing_originator = serde_json::json!({
            "baseUrl": "https://chatgpt.com/backend-api/codex",
            "headers": {
                "User-Agent": "codex_cli_rs/0.1"
            }
        });
        assert!(!is_chatgpt_codex_backend_value(&missing_originator));
    }

    #[test]
    fn provider_line_name_separates_official_and_codex() {
        assert_eq!(
            provider_line_name(&make_payload(
                "https://api.openai.com/v1",
                Some("codex_cli_rs")
            )),
            CHATGPT_OFFICIAL_API_PROFILE
        );
        assert_eq!(
            provider_line_name(&make_payload(
                "https://chatgpt.com/backend-api/codex",
                Some("codex_cli_rs")
            )),
            CHATGPT_CODEX_BACKEND_PROFILE
        );
    }
}
