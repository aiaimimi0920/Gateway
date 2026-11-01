//! Google Flow is independent of Gemini Canvas and FlowMusic; generation is not accepted yet.
use super::ProviderPreset;
use crate::routing::candidate::ProviderExecutionMode;
use std::collections::HashMap;

pub fn flow_web_preset() -> ProviderPreset {
    ProviderPreset {
        id: "flow-web".into(),
        adapter: "flow_web_compatible".into(),
        headers: HashMap::new(),
        extra_body: HashMap::new(),
        default_model: None,
        auth_mode: None,
        anthropic_version: None,
        beta_headers: None,
        auth_header_name: None,
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
