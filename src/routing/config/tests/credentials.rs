use super::*;

#[test]
fn single_credential_backward_compatible() {
    // When `credentials` is absent, the provider behaves as before:
    // one payload, empty credential_pool.
    let yaml = r#"
providers:
  - id: ai-hub
    preset: openai
    base_url: "https://example.com"
    api_key: "key123"
model_routes: []
"#;
    let config: RouteConfigYaml = serde_yaml::from_str(yaml).expect("parse");
    let inner = compile_yaml(config).expect("compile");
    assert_eq!(inner.providers.len(), 1);
    let p = &inner.providers[0];
    assert!(p.credential_pool.is_empty());
    assert_eq!(p.payload.api_key, "key123");
}

#[test]
fn multi_credential_provider_builds_pool() {
    let yaml = r#"
providers:
  - id: accio
    preset: accio
    base_url: "https://phoenix-gw.alibaba.com"
    supported_models: [claude-sonnet-4-6]
    credentials:
      - id: acc-001
        extra_body:
          token: "token_1"
        headers:
          utdid: "utd-001"
      - id: acc-002
        extra_body:
          token: "token_2"
        headers:
          utdid: "utd-002"
      - id: acc-003
        extra_body:
          token: "token_3"
        headers:
          utdid: "utd-003"
model_routes: []
"#;
    let config: RouteConfigYaml = serde_yaml::from_str(yaml).expect("parse");
    let inner = compile_yaml(config).expect("compile");
    assert_eq!(inner.providers.len(), 1, "only one provider, not three");
    let p = &inner.providers[0];
    assert_eq!(p.credential_pool.len(), 3);
    assert_eq!(p.credential_pool[0].id, "acc-001");
    assert_eq!(p.credential_pool[1].id, "acc-002");
    assert_eq!(p.credential_pool[2].id, "acc-003");
}

#[test]
fn credential_merges_headers_and_extra_body_from_provider() {
    let yaml = r#"
providers:
  - id: accio
    preset: accio
    base_url: "https://phoenix-gw.alibaba.com"
    credentials:
      - id: acc-001
        extra_body:
          token: "token_1"
        headers:
          utdid: "utd-001"
model_routes: []
"#;
    let config: RouteConfigYaml = serde_yaml::from_str(yaml).expect("parse");
    let inner = compile_yaml(config).expect("compile");
    let cred = &inner.providers[0].credential_pool[0];

    // Credential-specific header present
    assert_eq!(cred.payload.headers.get("utdid").unwrap(), "utd-001");
    // Preset headers inherited (accio preset carries the runtime app version).
    assert_eq!(cred.payload.headers.get("version").unwrap(), "0.5.6");
    assert!(cred.payload.headers.get("appKey").is_none());

    // Credential-specific extra_body present
    let extra = cred.payload.extra_body.as_ref().unwrap();
    assert_eq!(
        extra.get("token").unwrap(),
        &Value::String("token_1".to_string())
    );
}

#[test]
fn round_robin_cycles_through_credentials() {
    let yaml = r#"
providers:
  - id: pool-test
    base_url: "https://example.com"
    credentials:
      - id: c0
        api_key: "key-0"
      - id: c1
        api_key: "key-1"
      - id: c2
        api_key: "key-2"
model_routes: []
"#;
    let config: RouteConfigYaml = serde_yaml::from_str(yaml).expect("parse");
    let inner = compile_yaml(config).expect("compile");
    let p = &inner.providers[0];
    assert_eq!(p.credential_pool.len(), 3);

    // Call select_credential 6 times and verify round-robin cycling.
    let keys: Vec<String> = (0..6)
        .map(|_| {
            select_credential(p, None, None, None)
                .expect("credential")
                .api_key
        })
        .collect();
    assert_eq!(
        keys,
        vec!["key-0", "key-1", "key-2", "key-0", "key-1", "key-2"]
    );
}

#[test]
fn credential_selection_matches_xfyun_translated_model_names() {
    let yaml = r#"
providers:
  - id: xfyun-platform
    preset: xfyun
    base_url: "https://maas-api.cn-huabei-1.xf-yun.com"
    credentials:
      - id: hunyuan-key
        api_key: "key-hunyuan"
        supported_models: [xophunyuan7bmt]
      - id: qwen-key
        api_key: "key-qwen"
        supported_models: [xop35qwen2b]
    model_map:
      hunyuan-mt-7b: "xophunyuan7bmt"
      qwen-2b: "xop35qwen2b"
model_routes:
  - pattern: "hunyuan-*"
    provider_ids: [xfyun-platform]
    priority: 10
  - pattern: "qwen-2b"
    provider_ids: [xfyun-platform]
    priority: 10
"#;
    let store = make_store_from_yaml(yaml);

    let hunyuan = store.resolve_candidates(Some("hunyuan-mt-7b"));
    assert_eq!(hunyuan.len(), 1);
    assert_eq!(hunyuan[0].payload.api_key, "key-hunyuan");
    assert_eq!(hunyuan[0].upstream_model.as_deref(), Some("xophunyuan7bmt"));

    let qwen = store.resolve_candidates(Some("qwen-2b"));
    assert_eq!(qwen.len(), 1);
    assert_eq!(qwen[0].payload.api_key, "key-qwen");
    assert_eq!(qwen[0].upstream_model.as_deref(), Some("xop35qwen2b"));
}

#[test]
fn all_credentials_share_same_provider_account_id() {
    // Verifies that all candidates from a multi-credential provider
    // share the SAME provider_account_id → one AIMD controller.
    let yaml = r#"
providers:
  - id: shared-pool
    base_url: "https://example.com"
    credentials:
      - id: c0
        api_key: "key-0"
      - id: c1
        api_key: "key-1"
model_routes: []
"#;
    let store = make_store_from_yaml(yaml);
    // Resolve candidates multiple times — each call gets a different
    // credential, but the provider_account_id is always "shared-pool".
    let c1 = store.resolve_candidates(None);
    let c2 = store.resolve_candidates(None);
    assert_eq!(c1.len(), 1);
    assert_eq!(c2.len(), 1);
    assert_eq!(c1[0].provider_account_id, "shared-pool");
    assert_eq!(c2[0].provider_account_id, "shared-pool");
    // Different credentials selected (round-robin)
    assert_ne!(c1[0].payload.api_key, c2[0].payload.api_key);
}

#[test]
fn credential_auto_id_when_absent() {
    let yaml = r#"
providers:
  - id: my-provider
    base_url: "https://example.com"
    credentials:
      - api_key: "key-a"
      - api_key: "key-b"
model_routes: []
"#;
    let config: RouteConfigYaml = serde_yaml::from_str(yaml).expect("parse");
    let inner = compile_yaml(config).expect("compile");
    let pool = &inner.providers[0].credential_pool;
    assert_eq!(pool[0].id, "my-provider-cred-0");
    assert_eq!(pool[1].id, "my-provider-cred-1");
}

#[test]
fn compile_preset_plus_multi_credential() {
    // Verify that preset merging works with multi-credential.
    let yaml = r#"
providers:
  - id: codex-pool
    preset: codex
    base_url: "https://chatgpt.com/backend-api/codex"
    headers:
      Chatgpt-Account-Id: "shared-acc"
    credentials:
      - id: tok-a
        api_key: "tok_aaa"
      - id: tok-b
        api_key: "tok_bbb"
model_routes: []
"#;
    let config: RouteConfigYaml = serde_yaml::from_str(yaml).expect("parse");
    let inner = compile_yaml(config).expect("compile");
    let p = &inner.providers[0];
    assert_eq!(p.credential_pool.len(), 2);

    // Both credentials should have preset headers (User-Agent, Originator)
    // AND provider-level header (Chatgpt-Account-Id).
    for cred in &p.credential_pool {
        assert!(
            cred.payload.headers.contains_key("User-Agent"),
            "preset header missing"
        );
        assert!(
            cred.payload.headers.contains_key("Originator"),
            "preset header missing"
        );
        assert_eq!(
            cred.payload.headers.get("Chatgpt-Account-Id").unwrap(),
            "shared-acc"
        );
    }
    // Different api_keys
    assert_eq!(p.credential_pool[0].payload.api_key, "tok_aaa");
    assert_eq!(p.credential_pool[1].payload.api_key, "tok_bbb");
}

#[test]
fn credential_pool_propagates_session_auth_and_keepalive() {
    let yaml = r#"
providers:
  - id: grok-pool
    preset: grok
    base_url: "https://grok.com"
    credentials:
      - id: grok-session-1
        api_key: "sso-token"
        keepalive:
          service_url: "http://grok-keeper:8080"
          refresh_before_secs: 120
model_routes:
  - pattern: "grok-*"
    provider_ids: [grok-pool]
    priority: 10
"#;
    let store = make_store_from_yaml(yaml);
    let cands = store.resolve_candidates(Some("grok-3"));
    assert_eq!(cands.len(), 1);
    let payload = &cands[0].payload;
    assert_eq!(payload.credential_id.as_deref(), Some("grok-session-1"));
    assert_eq!(
        payload
            .session_auth
            .as_ref()
            .map(|cfg| cfg.primary_cookie_name()),
        Some("sso")
    );
    assert_eq!(
        payload
            .keepalive
            .as_ref()
            .map(|cfg| cfg.service_url.as_str()),
        Some("http://grok-keeper:8080")
    );
}

#[test]
fn provider_count_is_one_for_multi_credential() {
    // With 3 credentials, provider_count should still be 1.
    let yaml = r#"
providers:
  - id: multi
    base_url: "https://example.com"
    credentials:
      - api_key: "k1"
      - api_key: "k2"
      - api_key: "k3"
model_routes: []
"#;
    let store = make_store_from_yaml(yaml);
    assert_eq!(store.provider_count(), 1);
}

#[test]
fn credential_api_key_overrides_provider_key() {
    // When credentials array has api_key, each credential uses its own.
    // When a credential omits api_key, it falls back to the provider's.
    let yaml = r#"
providers:
  - id: mixed
    base_url: "https://example.com"
    api_key: "provider-key"
    credentials:
      - id: with-key
        api_key: "cred-key"
      - id: without-key
model_routes: []
"#;
    let config: RouteConfigYaml = serde_yaml::from_str(yaml).expect("parse");
    let inner = compile_yaml(config).expect("compile");
    let pool = &inner.providers[0].credential_pool;
    assert_eq!(pool[0].payload.api_key, "cred-key");
    assert_eq!(pool[1].payload.api_key, "provider-key");
}

#[test]
fn disabled_credentials_are_never_selected_and_all_disabled_provider_is_skipped() {
    let yaml = r#"
providers:
  - id: credential-status
    base_url: "https://example.com"
    credentials:
      - id: disabled-account
        api_key: "disabled-key"
        enabled: false
      - id: enabled-account
        api_key: "enabled-key"
model_routes: []
"#;
    let store = make_store_from_yaml(yaml);

    for _ in 0..12 {
        let candidates = store.resolve_candidates(None);
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].payload.api_key, "enabled-key");
    }

    let all_disabled_yaml = yaml
        .replace("enabled: false\n", "enabled: false\n")
        .replace(
            "      - id: enabled-account\n        api_key: \"enabled-key\"",
            "      - id: enabled-account\n        api_key: \"enabled-key\"\n        enabled: false",
        );
    let all_disabled = make_store_from_yaml(&all_disabled_yaml);
    assert!(all_disabled.resolve_candidates(None).is_empty());
}
