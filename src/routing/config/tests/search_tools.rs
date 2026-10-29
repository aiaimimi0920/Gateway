//! Generic tools must resolve to concrete, authorized operations before ranking.
use super::*;

fn tool_store() -> RouteConfigStore {
    make_store_from_yaml(
        r#"
providers:
  - id: you
    preset: you
    base_url: https://ydc-index.io
    supported_models: [you-search, you-fetch]
    credentials:
      - {id: you-key, api_key: synthetic, supported_models: [you-search]}
  - id: exa
    preset: exa
    base_url: https://api.exa.ai
    supported_models: [exa-search, exa-fetch]
    credentials:
      - {id: exa-key, api_key: synthetic, supported_models: [exa-search, exa-fetch]}
  - id: chat
    preset: openai
    base_url: https://example.com
    api_key: synthetic
account_groups:
  - {id: restricted, name: restricted, provider_credential_ids: [you-key]}
model_routes: []
"#,
    )
}

#[test]
fn generic_search_tools_resolve_concrete_models_without_chat_fallback() {
    let store = tool_store();
    let search = store.resolve_candidates(Some("search"));
    assert_eq!(search.len(), 2);
    assert!(search.iter().all(|c| matches!(
        c.upstream_model.as_deref(),
        Some("you-search" | "exa-search")
    )));
    let fetch = store.resolve_candidates(Some("fetch"));
    assert_eq!(fetch.len(), 1);
    assert_eq!(fetch[0].upstream_model.as_deref(), Some("exa-fetch"));
    assert!(store.list_models().iter().any(|m| m.id == "search"));
    assert!(store.list_models().iter().any(|m| m.id == "fetch"));
}

#[test]
fn generic_search_tools_preserve_groups_and_credential_model_limits() {
    let snapshot = tool_store().snapshot();
    let candidates = snapshot
        .resolve_candidates_for_account_group(Some("search"), Some("restricted"))
        .unwrap();
    assert_eq!(candidates.len(), 1);
    assert_eq!(
        candidates[0].payload.credential_id.as_deref(),
        Some("you-key")
    );
    assert!(snapshot
        .resolve_candidates_for_account_group(Some("fetch"), Some("restricted"))
        .unwrap()
        .is_empty());
}

#[test]
fn generic_search_tools_fail_closed_without_a_search_provider() {
    let store = make_store_from_yaml("providers: [{id: chat, preset: openai, base_url: 'https://example.com', api_key: synthetic}]\nmodel_routes: []");
    assert!(store.resolve_candidates(Some("search")).is_empty());
    assert!(store.resolve_candidates(Some("fetch")).is_empty());
}

#[test]
fn generic_search_tools_authorize_filtered_pool_fallback_not_fuzzy_aliases() {
    let snapshot = tool_store().snapshot();
    let resolution = snapshot.resolve_candidates_with_constraint(Some("search"), None);
    assert_eq!(
        resolution.explicit_provider_ids.unwrap(),
        vec!["you", "exa"]
    );
    let store = make_store_from_yaml("providers: [{id: chat, preset: openai, base_url: 'https://example.com', api_key: synthetic}]\nmodel_routes: []\naliases: {searching: gpt-test}");
    assert!(store.resolve_alias(Some("search")).is_none());
    assert!(store.resolve_candidates(Some("search")).is_empty());
}
