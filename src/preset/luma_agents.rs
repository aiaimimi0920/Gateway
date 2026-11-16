//! The official Agents product has static Keys and no browser session or implicit model.
use super::ProviderPreset;
use crate::{protocol::luma_agents as agents, routing::candidate::ProviderExecutionMode};
use std::collections::HashMap;

pub fn lumalabs_agents_api_preset() -> ProviderPreset {
    ProviderPreset {
        id: agents::PRESET_ID.into(),
        adapter: agents::ADAPTER.into(),
        headers: HashMap::new(),
        extra_body: HashMap::new(),
        default_model: None,
        auth_mode: Some("bearer".into()),
        anthropic_version: None,
        beta_headers: None,
        auth_header_name: Some("authorization".into()),
        responses_path: None,
        chat_completions_path: None,
        messages_path: None,
        search_path: None,
        fetch_path: None,
        research_path: None,
        balance_path: None,
        search_query_field: None,
        fetch_urls_field: None,
        session_auth: None,
        execution_mode: Some(ProviderExecutionMode::DirectHttp),
        endpoint_execution_modes: None,
    }
}
