use super::ProviderPreset;
use crate::routing::candidate::SEARCH_API_COMPATIBLE_ADAPTER;
use std::collections::HashMap;

/// Returns the built-in "linkup" preset for Linkup's search API.
pub fn linkup_preset() -> ProviderPreset {
    ProviderPreset {
        id: "linkup".to_string(),
        adapter: SEARCH_API_COMPATIBLE_ADAPTER.to_string(),
        headers: HashMap::new(),
        extra_body: HashMap::new(),
        default_model: Some("linkup-search".to_string()),
        auth_mode: None,
        anthropic_version: None,
        beta_headers: None,
        auth_header_name: None,
        responses_path: None,
        chat_completions_path: None,
        messages_path: None,
        search_path: Some("/v1/search".to_string()),
        fetch_path: Some("/v1/fetch".to_string()),
        research_path: Some("/v1/research".to_string()),
        balance_path: Some("/v1/credits/balance".to_string()),
        search_query_field: Some("q".to_string()),
        fetch_urls_field: Some("url".to_string()),
        session_auth: None,
        execution_mode: None,
        endpoint_execution_modes: None,
    }
}

/// Returns the built-in "perplexity-search" preset for Perplexity's Search API.
pub fn perplexity_search_preset() -> ProviderPreset {
    ProviderPreset {
        id: "perplexity-search".to_string(),
        adapter: SEARCH_API_COMPATIBLE_ADAPTER.to_string(),
        headers: HashMap::new(),
        extra_body: HashMap::new(),
        default_model: Some("perplexity-search".to_string()),
        auth_mode: None,
        anthropic_version: None,
        beta_headers: None,
        auth_header_name: None,
        responses_path: None,
        chat_completions_path: None,
        messages_path: None,
        search_path: Some("/search".to_string()),
        fetch_path: None,
        research_path: None,
        balance_path: None,
        search_query_field: Some("query".to_string()),
        fetch_urls_field: None,
        session_auth: None,
        execution_mode: None,
        endpoint_execution_modes: None,
    }
}

/// Returns the built-in "tavily" preset for Tavily's search API.
pub fn tavily_preset() -> ProviderPreset {
    ProviderPreset {
        id: "tavily".to_string(),
        adapter: SEARCH_API_COMPATIBLE_ADAPTER.to_string(),
        headers: HashMap::new(),
        extra_body: HashMap::new(),
        default_model: Some("tavily-search".to_string()),
        auth_mode: None,
        anthropic_version: None,
        beta_headers: None,
        auth_header_name: None,
        responses_path: None,
        chat_completions_path: None,
        messages_path: None,
        search_path: Some("/search".to_string()),
        fetch_path: None,
        research_path: None,
        balance_path: None,
        search_query_field: Some("query".to_string()),
        fetch_urls_field: None,
        session_auth: None,
        execution_mode: None,
        endpoint_execution_modes: None,
    }
}

/// Returns the built-in "you" preset for You.com's search API.
pub fn you_preset() -> ProviderPreset {
    ProviderPreset {
        id: "you".to_string(),
        adapter: SEARCH_API_COMPATIBLE_ADAPTER.to_string(),
        headers: HashMap::new(),
        extra_body: HashMap::new(),
        default_model: Some("you-search".to_string()),
        auth_mode: None,
        anthropic_version: None,
        beta_headers: None,
        auth_header_name: Some("X-API-Key".to_string()),
        responses_path: None,
        chat_completions_path: None,
        messages_path: None,
        search_path: Some("/v1/search".to_string()),
        fetch_path: None,
        research_path: None,
        balance_path: None,
        search_query_field: Some("query".to_string()),
        fetch_urls_field: None,
        session_auth: None,
        execution_mode: None,
        endpoint_execution_modes: None,
    }
}

/// Returns the built-in "exa" preset for Exa's search + contents API.
pub fn exa_preset() -> ProviderPreset {
    ProviderPreset {
        id: "exa".to_string(),
        adapter: SEARCH_API_COMPATIBLE_ADAPTER.to_string(),
        headers: HashMap::new(),
        extra_body: HashMap::new(),
        default_model: Some("exa-search".to_string()),
        auth_mode: None,
        anthropic_version: None,
        beta_headers: None,
        auth_header_name: None,
        responses_path: None,
        chat_completions_path: None,
        messages_path: None,
        search_path: Some("/search".to_string()),
        fetch_path: Some("/contents".to_string()),
        research_path: None,
        balance_path: None,
        search_query_field: Some("query".to_string()),
        fetch_urls_field: Some("urls".to_string()),
        session_auth: None,
        execution_mode: None,
        endpoint_execution_modes: None,
    }
}

/// Returns the built-in "jina-search" preset for Jina Search Foundation SERP.
pub fn jina_search_preset() -> ProviderPreset {
    let mut headers = HashMap::new();
    headers.insert("Accept".to_string(), "application/json".to_string());

    ProviderPreset {
        id: "jina-search".to_string(),
        adapter: SEARCH_API_COMPATIBLE_ADAPTER.to_string(),
        headers,
        extra_body: HashMap::new(),
        default_model: Some("jina-search".to_string()),
        auth_mode: None,
        anthropic_version: None,
        beta_headers: None,
        auth_header_name: None,
        responses_path: None,
        chat_completions_path: None,
        messages_path: None,
        search_path: Some("/search".to_string()),
        fetch_path: None,
        research_path: None,
        balance_path: None,
        search_query_field: Some("q".to_string()),
        fetch_urls_field: None,
        session_auth: None,
        execution_mode: None,
        endpoint_execution_modes: None,
    }
}

/// Returns the built-in "jina-reader" preset for Jina Reader URL fetches.
pub fn jina_reader_preset() -> ProviderPreset {
    let mut headers = HashMap::new();
    headers.insert("Accept".to_string(), "application/json".to_string());

    ProviderPreset {
        id: "jina-reader".to_string(),
        adapter: SEARCH_API_COMPATIBLE_ADAPTER.to_string(),
        headers,
        extra_body: HashMap::new(),
        default_model: Some("jina-fetch".to_string()),
        auth_mode: None,
        anthropic_version: None,
        beta_headers: None,
        auth_header_name: None,
        responses_path: None,
        chat_completions_path: None,
        messages_path: None,
        search_path: None,
        fetch_path: Some("/".to_string()),
        research_path: None,
        balance_path: None,
        search_query_field: None,
        fetch_urls_field: Some("url".to_string()),
        session_auth: None,
        execution_mode: None,
        endpoint_execution_modes: None,
    }
}

/// Returns the built-in "websearchapi" preset for WebSearchAPI's AI search API.
pub fn websearchapi_preset() -> ProviderPreset {
    ProviderPreset {
        id: "websearchapi".to_string(),
        adapter: SEARCH_API_COMPATIBLE_ADAPTER.to_string(),
        headers: HashMap::new(),
        extra_body: HashMap::new(),
        default_model: Some("websearchapi-search".to_string()),
        auth_mode: None,
        anthropic_version: None,
        beta_headers: None,
        auth_header_name: None,
        responses_path: None,
        chat_completions_path: None,
        messages_path: None,
        search_path: Some("/ai-search".to_string()),
        fetch_path: None,
        research_path: None,
        balance_path: None,
        search_query_field: Some("query".to_string()),
        fetch_urls_field: None,
        session_auth: None,
        execution_mode: None,
        endpoint_execution_modes: None,
    }
}
