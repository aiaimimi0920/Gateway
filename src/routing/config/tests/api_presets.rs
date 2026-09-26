use super::*;

#[test]
fn yaml_parsing_with_openai_preset() {
    let yaml = r#"
providers:
  - id: openai-default
    preset: openai
    base_url: "https://api.openai.com"
    api_key: "sk-test"
model_routes: []
"#;
    let store = make_store_from_yaml(yaml);
    assert_eq!(store.provider_count(), 1);
    let candidates = store.resolve_candidates(None);
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].payload.adapter, "openai_compatible");
}

#[test]
fn yaml_parsing_with_nvidia_openai_preset() {
    let yaml = r#"
providers:
  - id: nvidia-live
    preset: nvidia-openai
    base_url: "https://integrate.api.nvidia.com"
    api_key: "nvapi-test"
    supported_models: [meta/llama-3.3-70b-instruct]
model_routes:
  - pattern: "nvidia-*"
    provider_ids: [nvidia-live]
    priority: 10
"#;
    let store = make_store_from_yaml(yaml);
    let candidates = store.resolve_candidates(Some("nvidia-fixture"));
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].payload.adapter, "openai_compatible");
}

#[test]
fn yaml_parsing_with_linkup_preset() {
    let yaml = r#"
providers:
  - id: linkup-main
    preset: linkup
    base_url: "https://api.linkup.so"
    api_key: "linkup-test"
    supported_models: [linkup-search]
model_routes:
  - pattern: "linkup-*"
    provider_ids: [linkup-main]
    priority: 10
"#;
    let store = make_store_from_yaml(yaml);
    let candidates = store.resolve_candidates(Some("linkup-search"));
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].payload.adapter, SEARCH_API_COMPATIBLE_ADAPTER);
    assert_eq!(
        candidates[0].payload.search_path.as_deref(),
        Some("/v1/search")
    );
    assert_eq!(
        candidates[0].payload.fetch_path.as_deref(),
        Some("/v1/fetch")
    );
    assert_eq!(
        candidates[0].payload.research_path.as_deref(),
        Some("/v1/research")
    );
    assert_eq!(
        candidates[0].payload.balance_path.as_deref(),
        Some("/v1/credits/balance")
    );
    assert_eq!(candidates[0].protocol_family, LINKUP_SEARCH_FAMILY);
}

#[test]
fn yaml_parsing_with_perplexity_preset() {
    let yaml = r#"
providers:
  - id: perplexity-main
    preset: perplexity
    base_url: "https://api.perplexity.ai"
    api_key: "pplx-test"
    supported_models: [sonar]
model_routes:
  - pattern: "sonar*"
    provider_ids: [perplexity-main]
    priority: 10
"#;
    let store = make_store_from_yaml(yaml);
    let candidates = store.resolve_candidates(Some("sonar"));
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].payload.adapter, "openai_compatible");
    assert_eq!(
        candidates[0].payload.chat_completions_path.as_deref(),
        Some("/chat/completions")
    );
    assert_eq!(
        candidates[0].payload.responses_path.as_deref(),
        Some("/v1/responses")
    );
    assert_eq!(candidates[0].protocol_family, "openai");
}

#[test]
fn yaml_parsing_with_perplexity_search_preset() {
    let yaml = r#"
providers:
  - id: perplexity-search-main
    preset: perplexity-search
    base_url: "https://api.perplexity.ai"
    api_key: "pplx-search-test"
    supported_models: [perplexity-search]
model_routes:
  - pattern: "perplexity-search"
    provider_ids: [perplexity-search-main]
    priority: 10
"#;
    let store = make_store_from_yaml(yaml);
    let candidates = store.resolve_candidates(Some("perplexity-search"));
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].payload.adapter, SEARCH_API_COMPATIBLE_ADAPTER);
    assert_eq!(
        candidates[0].payload.search_path.as_deref(),
        Some("/search")
    );
    assert_eq!(
        candidates[0].payload.search_query_field.as_deref(),
        Some("query")
    );
    assert_eq!(candidates[0].protocol_family, PERPLEXITY_SEARCH_FAMILY);
}

#[test]
fn yaml_legacy_search_adapter_alias_is_canonicalized() {
    let yaml = r#"
providers:
  - id: legacy-search
    adapter: linkup_compatible
    protocol_family: linkup
    base_url: "https://api.example.com"
    api_key: "legacy-key"
    search_path: "/search"
model_routes:
  - pattern: "legacy-search"
    provider_ids: [legacy-search]
    priority: 10
"#;
    let store = make_store_from_yaml(yaml);
    let candidates = store.resolve_candidates(Some("legacy-search"));
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].payload.adapter, SEARCH_API_COMPATIBLE_ADAPTER);
    assert_eq!(candidates[0].adapter, SEARCH_API_COMPATIBLE_ADAPTER);
    assert_eq!(
        candidates[0].payload.search_path.as_deref(),
        Some("/search")
    );
    assert_eq!(candidates[0].protocol_family, LINKUP_SEARCH_FAMILY);
}

#[test]
fn yaml_parsing_with_anthropic_preset() {
    let yaml = r#"
providers:
  - id: anthropic-default
    preset: anthropic
    base_url: "https://api.anthropic.com"
    api_key: "sk-ant-test"
model_routes: []
"#;
    let store = make_store_from_yaml(yaml);
    let candidates = store.resolve_candidates(None);
    assert_eq!(candidates[0].payload.adapter, "anthropic_compatible");
    assert_eq!(candidates[0].protocol_family, "anthropic");
}

#[test]
fn yaml_parsing_with_codex_preset() {
    let yaml = r#"
providers:
  - id: codex-main
    preset: codex
    base_url: "https://chatgpt.com/backend-api/codex"
    api_key: "tok_xxx"
    headers:
      Chatgpt-Account-Id: "acc-123"
model_routes: []
"#;
    let store = make_store_from_yaml(yaml);
    let candidates = store.resolve_candidates(None);
    assert_eq!(candidates[0].payload.adapter, "openai_compatible");
    assert!(candidates[0]
        .payload
        .headers
        .contains_key("Chatgpt-Account-Id"));
    assert!(candidates[0].payload.headers.contains_key("User-Agent"));
}

#[test]
fn yaml_parsing_with_xfyun_preset() {
    let yaml = r#"
providers:
  - id: xfyun-platform
    label: "xfyun platform"
    preset: xfyun
    base_url: "https://maas-api.cn-huabei-1.xf-yun.com"
    api_key: "xf-test"
    supported_models: [xophunyuan7bmt, xophunyuanocr]
    model_map:
      hunyuan-mt-7b: "xophunyuan7bmt"
      hunyuan-ocr: "xophunyuanocr"
model_routes:
  - pattern: "hunyuan-*"
    provider_ids: [xfyun-platform]
    priority: 10
"#;
    let store = make_store_from_yaml(yaml);
    let candidates = store.resolve_candidates(Some("hunyuan-mt-7b"));
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].label, "xfyun platform");
    assert_eq!(candidates[0].payload.adapter, "openai_compatible");
    assert_eq!(
        candidates[0].payload.chat_completions_path.as_deref(),
        Some("/v2/chat/completions")
    );
    assert_eq!(candidates[0].protocol_family, "openai");
    assert_eq!(
        candidates[0].upstream_model.as_deref(),
        Some("xophunyuan7bmt")
    );
}

#[test]
fn yaml_parsing_with_xfyun_websocket_preset() {
    let yaml = r#"
providers:
  - id: xfyun-ws-platform
    label: "xfyun websocket"
    preset: xfyun-websocket
    base_url: "wss://maas-api.cn-huabei-1.xf-yun.com"
    api_key: "xf-test"
    auth_token: "xf-secret"
    extra_body:
      appId: "xf-app"
      uid: "gateway"
    supported_models: [qwen-native-ws]
    model_map:
      qwen-native-ws: "xop35qwen2b"
model_routes:
  - pattern: "qwen-native-ws"
    provider_ids: [xfyun-ws-platform]
    priority: 10
"#;
    let store = make_store_from_yaml(yaml);
    let candidates = store.resolve_candidates(Some("qwen-native-ws"));
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].payload.adapter, "xfyun_websocket_compatible");
    assert_eq!(
        candidates[0].payload.chat_completions_path.as_deref(),
        Some("/v1.1/chat")
    );
    assert_eq!(candidates[0].protocol_family, "xfyun_websocket");
    assert_eq!(candidates[0].upstream_model.as_deref(), Some("xop35qwen2b"));
    assert_eq!(
        candidates[0]
            .payload
            .extra_body
            .as_ref()
            .and_then(|extra| extra.get("appId")),
        Some(&json!("xf-app"))
    );
}
