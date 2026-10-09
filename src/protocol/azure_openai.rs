use crate::protocol::canonical::EndpointKind;
use crate::protocol::registry::canonicalize_protocol_profile_key;
use crate::routing::candidate::ProviderAccountPayload;

pub const AZURE_OPENAI_PROFILE: &str = "azure_openai";
pub const AZURE_OPENAI_PRESET_ID: &str = "azure-openai";
pub const AZURE_OPENAI_CATALOG_KEY: &str = "azure-openai";

pub fn is_azure_openai_profile(value: &str) -> bool {
    matches!(
        canonicalize_protocol_profile_key(value).as_str(),
        AZURE_OPENAI_PROFILE | "azure"
    )
}

pub fn is_azure_openai_base_url(base_url: &str) -> bool {
    let normalized = base_url.trim_end_matches('/').to_ascii_lowercase();
    normalized.contains("openai.azure.com")
        || (normalized.contains("azure.com") && normalized.contains("/openai/"))
        || normalized.contains(".cognitiveservices.azure.com")
}

pub fn owns_payload(payload: &ProviderAccountPayload) -> bool {
    payload.adapter.trim() == "openai_compatible" && is_azure_openai_base_url(&payload.base_url)
}

pub fn provider_line_name(_payload: &ProviderAccountPayload) -> &'static str {
    AZURE_OPENAI_PROFILE
}

pub fn supports_endpoint(endpoint_kind: EndpointKind) -> bool {
    matches!(
        endpoint_kind,
        EndpointKind::ChatCompletions
            | EndpointKind::Responses
            | EndpointKind::Messages
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::routing::candidate::ProviderAccountPayload;
    use std::collections::HashMap;

    fn make_payload(base_url: &str) -> ProviderAccountPayload {
        ProviderAccountPayload {
            discovered_protocols: Vec::new(),
            adapter: "openai_compatible".to_string(),
            base_url: base_url.to_string(),
            api_key: "tok".to_string(),
            credential_id: None,
            expires_at: None,
            runtime_state_object_key: None,
            account_name: None,
            execution_mode: None,
            endpoint_execution_modes: None,
            default_model: None,
            headers: HashMap::new(),
            auth_mode: Some("api-key".to_string()),
            anthropic_version: None,
            beta_headers: None,
            auth_header_name: None,
            auth_token: None,
            responses_path: Some("/responses".to_string()),
            chat_completions_path: Some("/chat/completions".to_string()),
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
    fn recognizes_azure_profile_and_base_urls() {
        assert!(is_azure_openai_profile("azure_openai"));
        assert!(is_azure_openai_profile("azure"));
        assert!(is_azure_openai_base_url(
            "https://example.openai.azure.com/openai/v1"
        ));
        assert!(is_azure_openai_base_url(
            "https://example.cognitiveservices.azure.com/openai/deployments/gpt/chat/completions"
        ));
        assert!(owns_payload(&make_payload(
            "https://example.openai.azure.com/openai/v1"
        )));
    }
}
