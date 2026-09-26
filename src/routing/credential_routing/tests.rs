use super::*;
use crate::protocol::registry::{
    EXA_SEARCH_FAMILY, JINA_READER_FAMILY, JINA_SEARCH_FAMILY, LINKUP_SEARCH_FAMILY,
    OPENAI_CHAT_FAMILY, PERPLEXITY_SEARCH_FAMILY, WEBSEARCHAPI_SEARCH_FAMILY, YOU_SEARCH_FAMILY,
};
use serde_json::json;
use std::collections::HashMap;

mod api_presets;
mod media_sessions;
mod metadata;
mod search;

fn make_credential(
    id: &str,
    kind: CredentialKind,
    provider: &str,
    api_key: Option<&str>,
    base_url: Option<&str>,
    payload: Option<serde_json::Value>,
) -> CredentialEntry {
    CredentialEntry {
        id: id.to_string(),
        kind,
        project_id: "proj-1".to_string(),
        user_id: "user-1".to_string(),
        provider: provider.to_string(),
        api_key: api_key.map(String::from),
        api_base_url: base_url.map(String::from),
        headers: None,
        account_payload: payload,
        quota_total_tokens: None,
        quota_remaining_tokens: None,
        expires_at: None,
        created_at: "2024-01-01T00:00:00.000Z".to_string(),
        updated_at: "2024-01-01T00:00:00.000Z".to_string(),
    }
}
