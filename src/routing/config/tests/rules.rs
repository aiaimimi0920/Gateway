use super::*;

#[test]
fn glob_star_matches_everything() {
    assert!(glob_match("*", "anything"));
    assert!(glob_match("*", ""));
}

#[test]
fn glob_prefix_star_matches_suffix() {
    assert!(glob_match("claude-*", "claude-sonnet-4-6"));
    assert!(glob_match("claude-*", "claude-opus-4"));
    assert!(!glob_match("claude-*", "gpt-4o"));
    assert!(!glob_match("claude-*", "claude"));
}

#[test]
fn glob_suffix_star_matches_prefix() {
    assert!(glob_match("*-mini", "gpt-4o-mini"));
    assert!(!glob_match("*-mini", "gpt-4o"));
}

#[test]
fn glob_exact_match() {
    assert!(glob_match("gpt-4o", "gpt-4o"));
    assert!(!glob_match("gpt-4o", "gpt-4o-mini"));
}

#[test]
fn glob_o_star_matches_o_models() {
    assert!(glob_match("o*", "o1"));
    assert!(glob_match("o*", "o3-mini"));
    assert!(!glob_match("o*", "gpt-o1"));
}

#[test]
fn subst_env_replaces_known_var() {
    std::env::set_var("TEST_GW_KEY", "hello-world");
    let result = subst_env("Bearer ${TEST_GW_KEY}");
    assert_eq!(result, "Bearer hello-world");
    std::env::remove_var("TEST_GW_KEY");
}

#[test]
fn subst_env_leaves_unknown_var_intact() {
    let result = subst_env("${DEFINITELY_NOT_SET_XYZ}");
    assert_eq!(result, "${DEFINITELY_NOT_SET_XYZ}");
}

#[test]
fn subst_env_no_placeholders_unchanged() {
    let s = "https://api.openai.com";
    assert_eq!(subst_env(s), s);
}

#[test]
fn unresolved_optional_keepalive_env_is_disabled() {
    std::env::remove_var("AI_GATEWAY_KEEPALIVE_URL");
    let document: RouteConfigYaml = serde_yaml::from_str(
        r#"
providers:
  - id: keepalive-optional
    base_url: "https://provider.example/v1"
    api_key: "test-key"
    keepalive:
      service_url: "${AI_GATEWAY_KEEPALIVE_URL}"
model_routes: []
aliases: {}
"#,
    )
    .expect("keepalive fixture should parse");

    let store = RouteConfigStore::from_document(document)
        .expect("unconfigured optional keepalive should not invalidate routes");
    let candidate = store
        .resolve_candidates(None)
        .into_iter()
        .next()
        .expect("provider candidate should remain available");
    assert!(candidate.payload.keepalive.is_none());
}

#[test]
fn new_store_is_empty() {
    let store = RouteConfigStore::new();
    assert!(!store.has_routes());
    assert_eq!(store.provider_count(), 0);
    assert!(store.resolve_candidates(None).is_empty());
    assert!(store.list_models().is_empty());
}

#[test]
fn yaml_parsing_no_preset() {
    let yaml = r#"
providers:
  - id: my-openai
    base_url: "https://api.openai.com"
    api_key: "sk-test"
    supported_models:
      - gpt-4o
model_routes: []
aliases: {}
"#;
    let store = make_store_from_yaml(yaml);
    assert!(store.has_routes());
    assert_eq!(store.provider_count(), 1);
}

#[test]
fn yaml_unknown_preset_returns_error() {
    let yaml = r#"
providers:
  - id: mystery
    preset: nonexistent-preset
    base_url: "https://example.com"
    api_key: "k"
model_routes: []
"#;
    let config: RouteConfigYaml = serde_yaml::from_str(yaml).unwrap();
    let result = compile_yaml(config);
    assert!(result.is_err());
}

#[test]
fn alias_resolution_known_alias() {
    let yaml = r#"
providers:
  - id: anthropic-default
    preset: anthropic
    base_url: "https://api.anthropic.com"
    api_key: "k"
model_routes: []
aliases:
  sonnet: claude-sonnet-4-6
"#;
    let store = make_store_from_yaml(yaml);
    assert_eq!(
        store.resolve_alias(Some("sonnet")),
        Some("claude-sonnet-4-6".to_string())
    );
}

#[test]
fn alias_resolution_unknown_returns_none() {
    let store = RouteConfigStore::new();
    assert!(store.resolve_alias(Some("unknown")).is_none());
    assert!(store.resolve_alias(None).is_none());
}

#[test]
fn alias_resolution_maps_search_api_virtual_models_to_legacy_route_models() {
    let store = RouteConfigStore::new();
    assert_eq!(
        store.resolve_alias(Some(SEARCH_ROUTE_MODEL)),
        Some(LEGACY_SEARCH_ROUTE_MODEL.to_string())
    );
    assert_eq!(
        store.resolve_alias(Some(FETCH_ROUTE_MODEL)),
        Some(LEGACY_FETCH_ROUTE_MODEL.to_string())
    );
    assert_eq!(
        store.resolve_alias(Some(RESEARCH_ROUTE_MODEL)),
        Some(LEGACY_RESEARCH_ROUTE_MODEL.to_string())
    );
    assert_eq!(
        store.resolve_alias(Some(BALANCE_ROUTE_MODEL)),
        Some(LEGACY_BALANCE_ROUTE_MODEL.to_string())
    );
}
