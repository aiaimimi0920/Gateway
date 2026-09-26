use super::*;

#[test]
fn converts_linkup_credential_to_search_adapter() {
    let cred = make_credential(
        "cred-linkup",
        CredentialKind::AccountCredential,
        "linkup",
        Some("linkup-key"),
        Some("https://api.linkup.so"),
        Some(json!({
            "preset": "linkup"
        })),
    );
    let candidate = credential_to_candidate(&cred, "linkup-search").unwrap();
    assert_eq!(candidate.adapter, SEARCH_API_COMPATIBLE_ADAPTER);
    assert_eq!(candidate.protocol_family, LINKUP_SEARCH_FAMILY);
    assert_eq!(candidate.payload.search_path.as_deref(), Some("/v1/search"));
    assert_eq!(candidate.payload.fetch_path.as_deref(), Some("/v1/fetch"));
    assert_eq!(
        candidate.payload.research_path.as_deref(),
        Some("/v1/research")
    );
    assert_eq!(
        candidate.payload.balance_path.as_deref(),
        Some("/v1/credits/balance")
    );
}

#[test]
fn converts_exa_credential_to_search_fetch_adapter() {
    let cred = make_credential(
        "cred-exa",
        CredentialKind::AccountCredential,
        "exa",
        Some("exa-key"),
        Some("https://api.exa.ai"),
        Some(json!({
            "preset": "exa"
        })),
    );
    let candidate = credential_to_candidate(&cred, "exa-search").unwrap();
    assert_eq!(candidate.adapter, SEARCH_API_COMPATIBLE_ADAPTER);
    assert_eq!(candidate.protocol_family, EXA_SEARCH_FAMILY);
    assert_eq!(candidate.payload.search_path.as_deref(), Some("/search"));
    assert_eq!(candidate.payload.fetch_path.as_deref(), Some("/contents"));
    assert!(candidate.payload.research_path.is_none());
    assert!(candidate.payload.balance_path.is_none());
    assert_eq!(
        candidate.payload.search_query_field.as_deref(),
        Some("query")
    );
    assert_eq!(candidate.payload.fetch_urls_field.as_deref(), Some("urls"));
}

#[test]
fn converts_jina_search_credential_to_search_adapter() {
    let cred = make_credential(
        "cred-jina-search",
        CredentialKind::AccountCredential,
        "jina",
        Some("jina-key"),
        Some("https://s.jina.ai"),
        Some(json!({
            "preset": "jina-search"
        })),
    );
    let candidate = credential_to_candidate(&cred, "jina-search").unwrap();
    assert_eq!(candidate.adapter, SEARCH_API_COMPATIBLE_ADAPTER);
    assert_eq!(candidate.protocol_family, JINA_SEARCH_FAMILY);
    assert_eq!(candidate.payload.search_path.as_deref(), Some("/search"));
    assert!(candidate.payload.fetch_path.is_none());
    assert_eq!(candidate.payload.search_query_field.as_deref(), Some("q"));
    assert_eq!(
        candidate.payload.headers.get("Accept").map(String::as_str),
        Some("application/json")
    );
}

#[test]
fn converts_jina_reader_credential_to_fetch_adapter() {
    let cred = make_credential(
        "cred-jina-reader",
        CredentialKind::AccountCredential,
        "jina-reader",
        Some("jina-key"),
        Some("https://r.jina.ai"),
        Some(json!({
            "preset": "jina-reader"
        })),
    );
    let candidate = credential_to_candidate(&cred, "jina-fetch").unwrap();
    assert_eq!(candidate.adapter, SEARCH_API_COMPATIBLE_ADAPTER);
    assert_eq!(candidate.protocol_family, JINA_READER_FAMILY);
    assert!(candidate.payload.search_path.is_none());
    assert_eq!(candidate.payload.fetch_path.as_deref(), Some("/"));
    assert_eq!(candidate.payload.fetch_urls_field.as_deref(), Some("url"));
    assert_eq!(
        candidate.payload.headers.get("Accept").map(String::as_str),
        Some("application/json")
    );
}

#[test]
fn converts_websearchapi_credential_to_search_adapter() {
    let cred = make_credential(
        "cred-websearchapi",
        CredentialKind::AccountCredential,
        "websearchapi",
        Some("websearchapi-key"),
        Some("https://api.websearchapi.ai"),
        Some(json!({
            "preset": "websearchapi"
        })),
    );
    let candidate = credential_to_candidate(&cred, "websearchapi-search").unwrap();
    assert_eq!(candidate.adapter, SEARCH_API_COMPATIBLE_ADAPTER);
    assert_eq!(candidate.protocol_family, WEBSEARCHAPI_SEARCH_FAMILY);
    assert_eq!(candidate.payload.search_path.as_deref(), Some("/ai-search"));
    assert_eq!(
        candidate.payload.search_query_field.as_deref(),
        Some("query")
    );
    assert!(candidate.payload.fetch_path.is_none());
    assert!(candidate.payload.research_path.is_none());
    assert!(candidate.payload.balance_path.is_none());
    assert!(candidate.payload.fetch_urls_field.is_none());
}

#[test]
fn converts_you_credential_to_search_adapter() {
    let cred = make_credential(
        "cred-you",
        CredentialKind::AccountCredential,
        "you",
        Some("you-key"),
        Some("https://ydc-index.io"),
        Some(json!({
            "preset": "you"
        })),
    );
    let candidate = credential_to_candidate(&cred, "you-search").unwrap();
    assert_eq!(candidate.adapter, SEARCH_API_COMPATIBLE_ADAPTER);
    assert_eq!(candidate.protocol_family, YOU_SEARCH_FAMILY);
    assert_eq!(candidate.payload.search_path.as_deref(), Some("/v1/search"));
    assert_eq!(
        candidate.payload.auth_header_name.as_deref(),
        Some("X-API-Key")
    );
}

#[test]
fn raw_perplexity_search_provider_uses_search_adapter_mapping() {
    let cred = make_credential(
        "cred-pplx-search",
        CredentialKind::AccountCredential,
        "perplexity-search",
        Some("pplx-key"),
        Some("https://api.perplexity.ai"),
        None,
    );
    let candidate = credential_to_candidate(&cred, "perplexity-search").unwrap();
    assert_eq!(candidate.adapter, SEARCH_API_COMPATIBLE_ADAPTER);
    assert_eq!(candidate.protocol_family, PERPLEXITY_SEARCH_FAMILY);
}
