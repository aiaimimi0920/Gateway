use super::*;

#[test]
fn resolve_candidates_claude_model_routes_to_anthropic() {
    let store = multi_provider_store();
    let cands = store.resolve_candidates(Some("claude-sonnet-4-6"));
    assert_eq!(cands.len(), 1);
    assert_eq!(cands[0].provider_account_id, "anthropic-default");
}

#[test]
fn resolve_candidates_gpt_model_routes_to_openai_and_codex() {
    let store = multi_provider_store();
    let cands = store.resolve_candidates(Some("gpt-4o"));
    assert_eq!(cands.len(), 2);
    let ids: Vec<&str> = cands
        .iter()
        .map(|c| c.provider_account_id.as_str())
        .collect();
    assert!(ids.contains(&"openai-default"));
    assert!(ids.contains(&"codex-main"));
}

#[test]
fn resolve_candidates_codex_model_higher_priority_route_first() {
    // "gpt-5.3-codex" matches both "gpt-*" (p=10) and "gpt-5.3-codex*" (p=20).
    // Higher priority route is returned first; deduplication keeps codex-main once.
    let store = multi_provider_store();
    let cands = store.resolve_candidates(Some("gpt-5.3-codex"));
    // codex-main comes first (from the p=20 route), then openai-default (from p=10).
    assert_eq!(cands[0].provider_account_id, "codex-main");
}

#[test]
fn resolve_candidates_no_model_returns_all() {
    let store = multi_provider_store();
    let cands = store.resolve_candidates(None);
    assert_eq!(cands.len(), 3);
}

#[test]
fn resolve_candidates_unrouted_model_prefers_exact_supported_provider() {
    let yaml = r#"
providers:
  - id: codex-main
    preset: codex
    base_url: "https://chatgpt.com/backend-api/codex"
    api_key: "tok"
    supported_models: [gpt-5.3-codex]
  - id: anthropic-default
    preset: anthropic
    base_url: "https://api.anthropic.com"
    api_key: "k-ant"
    supported_models: [claude-sonnet-4-6]
model_routes:
  - pattern: "gpt-5-codex-*"
    provider_ids: [codex-main]
    priority: 20
aliases:
  gpt-5-codex: gpt-5.3-codex
"#;
    let store = make_store_from_yaml(yaml);

    let cands = store.resolve_candidates(Some("gpt-5-codex"));

    assert_eq!(cands.len(), 1);
    assert_eq!(cands[0].provider_account_id, "codex-main");
    assert_eq!(cands[0].model_alias.as_deref(), Some("gpt-5-codex"));
    assert_eq!(cands[0].upstream_model.as_deref(), Some("gpt-5.3-codex"));
}

#[test]
fn resolve_candidates_unrouted_model_includes_provider_supporting_mapped_upstream_name() {
    let yaml = r#"
providers:
  - id: xfyun-platform
    preset: xfyun
    base_url: "https://maas-api.cn-huabei-1.xf-yun.com"
    api_key: "xf-test"
    supported_models: [xophunyuan7bmt]
    model_map:
      hunyuan-mt-7b: "xophunyuan7bmt"
  - id: canonical-provider
    base_url: "https://api.example.com"
    api_key: "canonical-test"
    supported_models: [hunyuan-mt-7b]
model_routes: []
"#;
    let store = make_store_from_yaml(yaml);

    let cands = store.resolve_candidates(Some("hunyuan-mt-7b"));
    let ids: Vec<&str> = cands
        .iter()
        .map(|candidate| candidate.provider_account_id.as_str())
        .collect();

    assert_eq!(ids, vec!["xfyun-platform", "canonical-provider"]);
    assert_eq!(cands[0].upstream_model.as_deref(), Some("xophunyuan7bmt"));
}

#[test]
fn resolve_candidates_unmatched_model_falls_back_to_all() {
    let store = multi_provider_store();
    // "llama-3" matches no route → all providers.
    let cands = store.resolve_candidates(Some("llama-3"));
    assert_eq!(cands.len(), 3);
}

#[test]
fn list_models_includes_supported_models_and_alias_keys_only() {
    let store = multi_provider_store();
    let models: Vec<String> = store.list_models().into_iter().map(|m| m.id).collect();
    // From supported_models
    assert!(models.contains(&"gpt-5.4".to_string()));
    assert!(models.contains(&"gpt-5.3-codex".to_string()));
    // From alias keys
    assert!(models.contains(&"sonnet".to_string()));
    assert!(models.contains(&"codex".to_string()));
    // Alias targets should stay hidden from the public catalog.
    assert!(!models.contains(&"claude-sonnet-4-6".to_string()));
}

#[test]
fn list_models_object_field_is_model() {
    let yaml = r#"
providers:
  - id: p
    base_url: "https://example.com"
    api_key: "k"
    supported_models: [my-model]
model_routes: []
"#;
    let store = make_store_from_yaml(yaml);
    let m = &store.list_models()[0];
    assert_eq!(m.object, "model");
    assert_eq!(m.owned_by, "neuro-gateway");
}
