use neuro_gateway::routing::candidate::RouteCandidate;
use neuro_gateway::routing::config::{RouteConfigStore, RouteConfigYaml};

fn store_from_yaml(yaml: &str) -> RouteConfigStore {
    let document: RouteConfigYaml = serde_yaml::from_str(yaml).expect("parse route fixture");
    RouteConfigStore::from_document(document).expect("compile route fixture")
}

fn candidate_with_credential_id(
    template: &RouteCandidate,
    provider_id: &str,
    credential_id: &str,
) -> RouteCandidate {
    let mut candidate = template.clone();
    candidate.provider_account_id = provider_id.to_string();
    candidate.provider_credential_id = Some(credential_id.to_string());
    candidate.payload.credential_id = Some(credential_id.to_string());
    candidate
}

#[test]
fn disabled_credentials_never_route_and_all_disabled_provider_is_skipped() {
    let store = store_from_yaml(
        r#"
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
"#,
    );

    for _ in 0..12 {
        let candidates = store.resolve_candidates(None);
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].payload.api_key, "enabled-key");
    }

    let all_disabled = store_from_yaml(
        r#"
providers:
  - id: credential-status
    base_url: "https://example.com"
    credentials:
      - id: disabled-account
        api_key: "disabled-key"
        enabled: false
      - id: disabled-account-2
        api_key: "disabled-key-2"
        enabled: false
model_routes: []
"#,
    );
    assert!(all_disabled.resolve_candidates(None).is_empty());
}

#[test]
fn account_group_constraints_apply_before_credential_round_robin() {
    let store = store_from_yaml(
        r#"
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
"#,
    );

    for _ in 0..20 {
        let candidates = store
            .resolve_candidates_for_account_group(None, Some("only-group"))
            .expect("group should resolve");
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].payload.api_key, "group-key");
    }
}

#[test]
fn disabled_credentials_remain_visible_in_account_inventory() {
    let store = store_from_yaml(
        r#"
providers:
  - id: inventory-provider
    label: "Inventory provider"
    base_url: "https://example.com"
    credentials:
      - id: inventory-disabled
        account_name: "Disabled account"
        api_key: "disabled-key"
        enabled: false
account_groups:
  - id: retained-group
    name: "Retained group"
    provider_credential_ids: [inventory-disabled]
model_routes: []
"#,
    );

    let inventory = store.account_group_inventory();
    let account = inventory
        .accounts
        .iter()
        .find(|account| account.id == "inventory-disabled")
        .expect("disabled account remains visible");
    assert!(!account.enabled);
    assert_eq!(account.group_ids, vec!["retained-group".to_string()]);

    let serialized = serde_json::to_value(inventory).expect("serialize inventory");
    let serialized_account = serialized["accounts"]
        .as_array()
        .and_then(|accounts| {
            accounts
                .iter()
                .find(|account| account["id"] == "inventory-disabled")
        })
        .expect("serialized disabled account");
    assert_eq!(serialized_account["enabled"], false);
}

#[test]
fn account_group_constraint_filters_concrete_candidates_from_every_runtime_source() {
    let store = store_from_yaml(
        r#"
providers:
  - id: source-fixture
    base_url: "https://example.com"
    credentials:
      - { id: access-live, api_key: access-live-key }
      - { id: access-outside, api_key: access-outside-key }
      - { id: redis-live, api_key: redis-live-key }
      - { id: redis-outside, api_key: redis-outside-key }
      - { id: postgres-live, api_key: postgres-live-key }
      - { id: postgres-outside, api_key: postgres-outside-key }
      - { id: yaml-live, api_key: yaml-live-key }
      - { id: yaml-outside, api_key: yaml-outside-key }
account_groups:
  - id: isolated
    name: Isolated
    provider_credential_ids: [access-live, redis-live, postgres-live, yaml-live]
model_routes: []
"#,
    );
    let template = store
        .resolve_candidates(None)
        .into_iter()
        .next()
        .expect("candidate template");
    let candidates = [
        ("access-provider", "access-outside"),
        ("access-provider", "access-live"),
        ("redis-provider", "redis-outside"),
        ("redis-provider", "redis-live"),
        ("postgres-provider", "postgres-outside"),
        ("postgres-provider", "postgres-live"),
        ("yaml-provider", "yaml-outside"),
        ("yaml-provider", "yaml-live"),
    ]
    .into_iter()
    .map(|(provider_id, credential_id)| {
        candidate_with_credential_id(&template, provider_id, credential_id)
    })
    .collect();

    let constraint = store
        .account_group_constraint(Some("isolated"))
        .expect("enabled account group");
    let filtered = constraint.filter_candidates(candidates);
    let retained_ids = filtered
        .iter()
        .filter_map(|candidate| candidate.payload.credential_id.as_deref())
        .collect::<Vec<_>>();

    assert_eq!(
        retained_ids,
        vec!["access-live", "redis-live", "postgres-live", "yaml-live"]
    );
}

#[test]
fn all_disabled_credentials_remove_provider_models_routes_and_aliases_from_discovery() {
    let store = store_from_yaml(
        r#"
providers:
  - id: disabled-provider
    base_url: "https://example.com"
    supported_models: [provider-disabled-model]
    credentials:
      - id: disabled-account
        api_key: disabled-key
        enabled: false
model_routes:
  - pattern: route-disabled-model
    provider_ids: [disabled-provider]
    priority: 10
aliases:
  disabled-alias: route-disabled-model
"#,
    );

    let models = store
        .list_models()
        .into_iter()
        .map(|model| model.id)
        .collect::<Vec<_>>();

    assert!(!models.contains(&"provider-disabled-model".to_string()));
    assert!(!models.contains(&"route-disabled-model".to_string()));
    assert!(!models.contains(&"disabled-alias".to_string()));
}

#[test]
fn provider_without_credentials_keeps_its_default_account_models_discoverable() {
    let store = store_from_yaml(
        r#"
providers:
  - id: default-account-provider
    base_url: "https://example.com"
    api_key: default-key
    supported_models: [default-account-model]
model_routes:
  - pattern: default-route-model
    provider_ids: [default-account-provider]
    priority: 10
aliases:
  default-alias: default-route-model
"#,
    );

    let models = store
        .list_models()
        .into_iter()
        .map(|model| model.id)
        .collect::<Vec<_>>();

    assert!(models.contains(&"default-account-model".to_string()));
    assert!(models.contains(&"default-route-model".to_string()));
    assert!(models.contains(&"default-alias".to_string()));
}
