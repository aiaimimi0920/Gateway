//! Consumption-site session executor; no shared authentication or Sonar API defaults.
use super::ProviderPreset;
use crate::{
    protocol::perplexity_web::material as product, routing::candidate::ProviderExecutionMode,
};
use std::collections::HashMap;

pub fn perplexity_web_preset() -> ProviderPreset {
    ProviderPreset {
        id: product::PRESET_ID.into(),
        adapter: product::ADAPTER.into(),
        auth_mode: Some(product::AUTH_MODE.into()),
        default_model: None,
        headers: HashMap::new(),
        extra_body: HashMap::new(),
        anthropic_version: None,
        beta_headers: None,
        auth_header_name: None,
        responses_path: None,
        chat_completions_path: Some(product::ASK_PATH.into()),
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
