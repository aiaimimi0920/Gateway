use super::*;

#[test]
fn perplexity_search_preset_uses_query_field() {
    let preset = perplexity_search_preset();
    assert_eq!(preset.adapter, SEARCH_API_COMPATIBLE_ADAPTER);
    assert_eq!(preset.default_model.as_deref(), Some("perplexity-search"));
    assert_eq!(preset.search_path.as_deref(), Some("/search"));
    assert_eq!(preset.search_query_field.as_deref(), Some("query"));
    assert!(preset.fetch_path.is_none());
    assert!(preset.research_path.is_none());
    assert!(preset.balance_path.is_none());
}

#[test]
fn linkup_preset_has_search_adapter_and_path() {
    let preset = linkup_preset();
    assert_eq!(preset.id, "linkup");
    assert_eq!(preset.adapter, SEARCH_API_COMPATIBLE_ADAPTER);
    assert_eq!(preset.default_model.as_deref(), Some("linkup-search"));
    assert_eq!(preset.search_path.as_deref(), Some("/v1/search"));
    assert_eq!(preset.fetch_path.as_deref(), Some("/v1/fetch"));
    assert_eq!(preset.research_path.as_deref(), Some("/v1/research"));
    assert_eq!(preset.balance_path.as_deref(), Some("/v1/credits/balance"));
    assert_eq!(preset.fetch_urls_field.as_deref(), Some("url"));
}

#[test]
fn tavily_preset_has_search_adapter_and_path() {
    let preset = tavily_preset();
    assert_eq!(preset.id, "tavily");
    assert_eq!(preset.adapter, SEARCH_API_COMPATIBLE_ADAPTER);
    assert_eq!(preset.default_model.as_deref(), Some("tavily-search"));
    assert_eq!(preset.search_path.as_deref(), Some("/search"));
    assert_eq!(preset.search_query_field.as_deref(), Some("query"));
    assert!(preset.fetch_path.is_none());
    assert!(preset.research_path.is_none());
    assert!(preset.balance_path.is_none());
}

#[test]
fn exa_preset_has_search_and_contents_paths() {
    let preset = exa_preset();
    assert_eq!(preset.id, "exa");
    assert_eq!(preset.adapter, SEARCH_API_COMPATIBLE_ADAPTER);
    assert_eq!(preset.default_model.as_deref(), Some("exa-search"));
    assert_eq!(preset.search_path.as_deref(), Some("/search"));
    assert_eq!(preset.fetch_path.as_deref(), Some("/contents"));
    assert!(preset.research_path.is_none());
    assert!(preset.balance_path.is_none());
    assert_eq!(preset.search_query_field.as_deref(), Some("query"));
    assert_eq!(preset.fetch_urls_field.as_deref(), Some("urls"));
}

#[test]
fn jina_search_preset_has_search_path_and_json_accept() {
    let preset = jina_search_preset();
    assert_eq!(preset.id, "jina-search");
    assert_eq!(preset.adapter, SEARCH_API_COMPATIBLE_ADAPTER);
    assert_eq!(preset.default_model.as_deref(), Some("jina-search"));
    assert_eq!(preset.search_path.as_deref(), Some("/search"));
    assert!(preset.fetch_path.is_none());
    assert_eq!(preset.search_query_field.as_deref(), Some("q"));
    assert_eq!(
        preset.headers.get("Accept").map(String::as_str),
        Some("application/json")
    );
}

#[test]
fn jina_reader_preset_has_fetch_path_and_json_accept() {
    let preset = jina_reader_preset();
    assert_eq!(preset.id, "jina-reader");
    assert_eq!(preset.adapter, SEARCH_API_COMPATIBLE_ADAPTER);
    assert_eq!(preset.default_model.as_deref(), Some("jina-fetch"));
    assert_eq!(preset.fetch_path.as_deref(), Some("/"));
    assert!(preset.search_path.is_none());
    assert_eq!(preset.fetch_urls_field.as_deref(), Some("url"));
    assert_eq!(
        preset.headers.get("Accept").map(String::as_str),
        Some("application/json")
    );
}

#[test]
fn you_preset_has_search_adapter_and_custom_auth_header() {
    let preset = you_preset();
    assert_eq!(preset.id, "you");
    assert_eq!(preset.adapter, SEARCH_API_COMPATIBLE_ADAPTER);
    assert_eq!(preset.default_model.as_deref(), Some("you-search"));
    assert_eq!(preset.search_path.as_deref(), Some("/v1/search"));
    assert!(preset.fetch_path.is_none());
    assert!(preset.research_path.is_none());
    assert!(preset.balance_path.is_none());
    assert_eq!(preset.search_query_field.as_deref(), Some("query"));
    assert_eq!(preset.auth_header_name.as_deref(), Some("X-API-Key"));
}

#[test]
fn websearchapi_preset_has_search_adapter_and_path() {
    let preset = websearchapi_preset();
    assert_eq!(preset.id, "websearchapi");
    assert_eq!(preset.adapter, SEARCH_API_COMPATIBLE_ADAPTER);
    assert_eq!(preset.default_model.as_deref(), Some("websearchapi-search"));
    assert_eq!(preset.search_path.as_deref(), Some("/ai-search"));
    assert_eq!(preset.search_query_field.as_deref(), Some("query"));
    assert!(preset.fetch_path.is_none());
    assert!(preset.research_path.is_none());
    assert!(preset.balance_path.is_none());
    assert!(preset.fetch_urls_field.is_none());
}
