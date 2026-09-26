use super::*;

#[test]
fn account_group_filters_credentials_before_round_robin_selection() {
    let yaml = r#"
providers:
  - id: grouped-pool
    base_url: "https://example.com"
    credentials:
      - id: group-account
        api_key: "group-key"
      - id: outside-account
        api_key: "outside-key"
account_groups:
  - id: only-group
    name: "Only group"
    provider_credential_ids: [group-account]
model_routes: []
"#;
    let store = make_store_from_yaml(yaml);

    for _ in 0..20 {
        let candidates = store
            .resolve_candidates_for_account_group(None, Some("only-group"))
            .expect("group should resolve");
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].payload.api_key, "group-key");
    }
}

#[test]
fn account_group_inventory_is_cached_per_immutable_snapshot() {
    let store = make_store_from_yaml(
        r#"
providers:
  - id: cached-provider
    label: "Cached Provider"
    base_url: "https://example.com/v1"
    api_key: "sk-test"
    credentials:
      - id: cached-account
        account_name: "Cached Account"
        api_key: "sk-account"
account_groups:
  - id: cached-group
    name: "Cached Group"
    provider_credential_ids: [cached-account]
model_routes: []
"#,
    );
    let snapshot = store.snapshot();

    assert!(snapshot.account_group_inventory_cache.get().is_none());
    let mut first = snapshot.account_group_inventory();
    assert!(snapshot.account_group_inventory_cache.get().is_some());
    first.accounts[0].display_name = "caller mutation".to_string();

    let second = snapshot.account_group_inventory();
    assert_eq!(second.accounts[0].display_name, "Cached Account");
}

#[test]
fn account_group_inventory_exposes_effective_multiplier_and_membership() {
    let yaml = r#"
providers:
  - id: openai-default
    label: "OpenAI"
    preset: openai
    base_url: "https://api.openai.com"
    api_key: "sk-openai"
    account_name: "OpenAI Shared"
    supported_models: [gpt-5.4]
  - id: codex-main
    label: "Codex"
    preset: codex
    base_url: "https://muyuan.do/v1"
    api_key: ""
    supported_models: [gpt-5.4]
    credentials:
      - id: codex-live
        api_key: "sk-codex"
        account_name: "Codex Live"
account_groups:
  - id: premium
    name: "Premium"
    billing_multiplier: 1.25
    provider_credential_ids: [openai-default::default, codex-live]
model_routes: []
"#;
    let store = make_store_from_yaml(yaml);
    let inventory = store.account_group_inventory();

    assert_eq!(inventory.account_groups.len(), 1);
    assert_eq!(inventory.account_groups[0].id, "premium");
    assert_eq!(inventory.account_groups[0].billing_multiplier, 1.25);
    assert_eq!(
        inventory.account_groups[0].configured_billing_multiplier,
        Some(1.25)
    );
    assert_eq!(inventory.account_groups[0].member_count, 2);
    assert_eq!(
        inventory.account_groups[0].providers,
        vec!["codex-main".to_string(), "openai-default".to_string()]
    );

    assert!(inventory.accounts.iter().any(|account| {
        account.id == "openai-default::default"
            && account.display_name == "OpenAI Shared"
            && account.vendor_key.is_none()
            && account.vendor_name.is_none()
            && account.group_ids == vec!["premium".to_string()]
    }));
    assert!(inventory.accounts.iter().any(|account| {
        account.id == "codex-live"
            && account.display_name == "Codex Live"
            && account.vendor_key.is_none()
            && account.vendor_name.is_none()
            && account.group_ids == vec!["premium".to_string()]
    }));
    assert!(inventory
        .providers
        .iter()
        .all(|provider| provider.vendor_key.is_none() && provider.vendor_name.is_none()));
}

#[test]
fn account_group_inventory_preserves_optional_vendor_metadata_with_camel_case_json() {
    let yaml = r#"
providers:
  - id: managed-provider
    label: "Managed OpenAI"
    vendor_key: muyuan
    vendor_name: "木元"
    preset: openai
    base_url: "https://api.example.com/v1"
    api_key: "sk-test"
    credentials:
      - id: managed-account
        api_key: "sk-account"
        account_name: "Managed Account"
model_routes: []
"#;
    let store = make_store_from_yaml(yaml);
    let inventory = store.account_group_inventory();
    let account = inventory
        .accounts
        .iter()
        .find(|account| account.id == "managed-account")
        .expect("credential account");
    let provider = inventory
        .providers
        .iter()
        .find(|provider| provider.id == "managed-provider")
        .expect("provider");

    assert_eq!(account.vendor_key.as_deref(), Some("muyuan"));
    assert_eq!(account.vendor_name.as_deref(), Some("木元"));
    assert_eq!(provider.vendor_key.as_deref(), Some("muyuan"));
    assert_eq!(provider.vendor_name.as_deref(), Some("木元"));

    let json = serde_json::to_value(&inventory).expect("inventory JSON");
    let json_account = json["accounts"]
        .as_array()
        .and_then(|accounts| accounts.iter().find(|item| item["id"] == "managed-account"))
        .expect("serialized account");
    let json_provider = json["providers"]
        .as_array()
        .and_then(|providers| {
            providers
                .iter()
                .find(|item| item["id"] == "managed-provider")
        })
        .expect("serialized provider");
    assert_eq!(json_account["vendorKey"], "muyuan");
    assert_eq!(json_account["vendorName"], "木元");
    assert_eq!(json_provider["vendorKey"], "muyuan");
    assert_eq!(json_provider["vendorName"], "木元");

    let legacy_document: RouteConfigYaml = serde_yaml::from_str(
        r#"
providers:
  - id: legacy-provider
    base_url: "https://legacy.example.com/v1"
    api_key: "sk-legacy"
model_routes: []
"#,
    )
    .expect("legacy route document");
    let legacy_json = serde_json::to_value(legacy_document).expect("legacy JSON");
    let legacy_provider = &legacy_json["providers"][0];
    assert!(legacy_provider.get("vendor_key").is_none());
    assert!(legacy_provider.get("vendor_name").is_none());
}

#[test]
fn resolve_candidates_for_account_group_filters_default_and_credential_accounts() {
    let yaml = r#"
providers:
  - id: openai-default
    label: "OpenAI"
    preset: openai
    base_url: "https://api.openai.com"
    api_key: "sk-openai"
    supported_models: [gpt-5.4]
  - id: codex-main
    label: "Codex"
    preset: codex
    base_url: "https://muyuan.do/v1"
    api_key: ""
    supported_models: [gpt-5.4]
    credentials:
      - id: codex-live
        api_key: "sk-codex"
      - id: codex-spare
        api_key: "sk-codex-2"
account_groups:
  - id: premium
    name: "Premium"
    provider_credential_ids: [openai-default::default, codex-live]
model_routes:
  - pattern: "gpt-*"
    provider_ids: [openai-default, codex-main]
    priority: 10
"#;
    let store = make_store_from_yaml(yaml);

    let candidates = store
        .resolve_candidates_for_account_group(Some("gpt-5.4"), Some("premium"))
        .expect("group candidates");
    let account_ids = candidates
        .iter()
        .map(candidate_account_group_member_id)
        .collect::<Vec<_>>();
    assert_eq!(
        account_ids,
        vec![
            "openai-default::default".to_string(),
            "codex-live".to_string()
        ]
    );
}

#[test]
fn resolve_candidates_for_account_group_rejects_unknown_or_disabled_group() {
    let yaml = r#"
providers:
  - id: openai-default
    preset: openai
    base_url: "https://api.openai.com"
    api_key: "sk-openai"
    supported_models: [gpt-5.4]
account_groups:
  - id: disabled
    name: "Disabled"
    enabled: false
    provider_credential_ids: [openai-default::default]
model_routes:
  - pattern: "gpt-*"
    provider_ids: [openai-default]
    priority: 10
"#;
    let store = make_store_from_yaml(yaml);

    let missing = store
        .resolve_candidates_for_account_group(Some("gpt-5.4"), Some("missing"))
        .expect_err("missing group should fail");
    assert_eq!(
        missing,
        RouteAccountGroupSelectionError::NotFound("missing".to_string())
    );

    let disabled = store
        .resolve_candidates_for_account_group(Some("gpt-5.4"), Some("disabled"))
        .expect_err("disabled group should fail");
    assert_eq!(
        disabled,
        RouteAccountGroupSelectionError::Disabled("disabled".to_string())
    );
}
