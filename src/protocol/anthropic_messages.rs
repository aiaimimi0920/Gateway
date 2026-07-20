use crate::protocol::canonical::EndpointKind;
use crate::protocol::registry::canonicalize_protocol_profile_key;
use crate::routing::candidate::ProviderAccountPayload;

pub const ANTHROPIC_MESSAGES_PROFILE: &str = "anthropic";
pub const ANTHROPIC_MESSAGES_PRESET_ID: &str = "anthropic";
pub const ANTHROPIC_MESSAGES_CATALOG_KEY: &str = "anthropic-compatible";

pub fn is_anthropic_messages_profile(value: &str) -> bool {
    matches!(
        canonicalize_protocol_profile_key(value).as_str(),
        ANTHROPIC_MESSAGES_PROFILE | "anthropic_messages"
    )
}

pub fn is_anthropic_messages_base_url(base_url: &str) -> bool {
    let normalized = base_url.trim_end_matches('/').to_ascii_lowercase();
    normalized.contains("api.anthropic.com")
        || normalized.contains("127.0.0.1")
        || normalized.contains("localhost")
        || normalized.contains("host.docker.internal")
}

pub fn owns_payload(payload: &ProviderAccountPayload) -> bool {
    payload.adapter.trim() == "anthropic_compatible"
        && is_anthropic_messages_base_url(&payload.base_url)
}

pub fn provider_line_name(_payload: &ProviderAccountPayload) -> &'static str {
    ANTHROPIC_MESSAGES_PROFILE
}

pub fn supports_endpoint(endpoint_kind: EndpointKind) -> bool {
    matches!(
        endpoint_kind,
        EndpointKind::ChatCompletions
            | EndpointKind::Responses
            | EndpointKind::Messages
            | EndpointKind::Completions
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::routing::candidate::ProviderAccountPayload;
    use std::collections::HashMap;

    fn make_payload(base_url: &str) -> ProviderAccountPayload {
        ProviderAccountPayload {
            adapter: "anthropic_compatible".to_string(),
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
            auth_mode: Some("x-api-key".to_string()),
            anthropic_version: Some("2023-06-01".to_string()),
            beta_headers: None,
            auth_header_name: None,
            auth_token: None,
            responses_path: None,
            chat_completions_path: None,
            completions_path: None,
            embeddings_path: None,
            audio_transcriptions_path: None,
            audio_speech_path: None,
            messages_path: Some("/v1/messages".to_string()),
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
    fn recognizes_anthropic_profile_and_base_url() {
        assert!(is_anthropic_messages_profile("anthropic"));
        assert!(is_anthropic_messages_profile("anthropic_messages"));
        assert!(is_anthropic_messages_base_url("https://api.anthropic.com"));
        assert!(is_anthropic_messages_base_url("http://127.0.0.1:42335"));
        assert!(owns_payload(&make_payload("https://api.anthropic.com")));
    }
}
