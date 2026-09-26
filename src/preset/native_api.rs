use super::ProviderPreset;
use serde_json::Value;
use std::collections::HashMap;

/// Returns the built-in "bedrock-converse" preset for AWS Bedrock Converse.
pub fn bedrock_converse_preset() -> ProviderPreset {
    ProviderPreset {
        id: "bedrock-converse".to_string(),
        adapter: "bedrock_converse_compatible".to_string(),
        headers: HashMap::new(),
        extra_body: HashMap::new(),
        default_model: None,
        auth_mode: None,
        anthropic_version: None,
        beta_headers: None,
        auth_header_name: Some("authorization".to_string()),
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
        execution_mode: None,
        endpoint_execution_modes: None,
    }
}

/// Returns the built-in "cohere-chat" preset for Cohere Chat.
pub fn cohere_chat_preset() -> ProviderPreset {
    ProviderPreset {
        id: "cohere-chat".to_string(),
        adapter: "cohere_compatible".to_string(),
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
        execution_mode: None,
        endpoint_execution_modes: None,
    }
}

/// Returns the built-in "xfyun-websocket" preset for XFYun Spark/MaaS native
/// signed WebSocket chat transport.
///
/// This preset is distinct from the OpenAI-compatible HTTP surface:
/// - auth is query-string signing based on `apiKey + apiSecret + appId`
/// - transport is WebSocket upstream
/// - the gateway still bridges it back to the caller-visible protocol family
pub fn xfyun_websocket_preset() -> ProviderPreset {
    let mut extra_body = HashMap::new();
    extra_body.insert("uid".to_string(), Value::String("gateway".to_string()));

    ProviderPreset {
        id: "xfyun-websocket".to_string(),
        adapter: "xfyun_websocket_compatible".to_string(),
        headers: HashMap::new(),
        extra_body,
        default_model: None,
        auth_mode: None,
        anthropic_version: None,
        beta_headers: None,
        auth_header_name: None,
        responses_path: None,
        chat_completions_path: Some("/v1.1/chat".to_string()),
        messages_path: None,
        search_path: None,
        fetch_path: None,
        research_path: None,
        balance_path: None,
        search_query_field: None,
        fetch_urls_field: None,
        session_auth: None,
        execution_mode: None,
        endpoint_execution_modes: None,
    }
}

/// Returns the built-in "anthropic" preset for Anthropic Claude API.
pub fn anthropic_preset() -> ProviderPreset {
    ProviderPreset {
        id: "anthropic".to_string(),
        adapter: "anthropic_compatible".to_string(),
        headers: HashMap::new(),
        extra_body: HashMap::new(),
        default_model: Some("claude-sonnet-4-6".to_string()),
        auth_mode: None,
        anthropic_version: Some("2023-06-01".to_string()),
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
        execution_mode: None,
        endpoint_execution_modes: None,
    }
}
