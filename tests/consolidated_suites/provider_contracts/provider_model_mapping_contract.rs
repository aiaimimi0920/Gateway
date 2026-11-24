use neuro_gateway::routing::config::{RouteConfigStore, RouteConfigYaml};
use serde_json::json;

fn mapped_store() -> RouteConfigStore {
    let config: RouteConfigYaml = serde_yaml::from_str(
        r#"
providers:
  - id: pool
    preset: openai
    base_url: http://127.0.0.1:1
    supported_models: [b, c]
    model_map: {old: b}
    model_map_targets: {a: [b, c]}
    credentials:
      - {id: b-account, api_key: b-secret, supported_models: [b]}
      - {id: c-account, api_key: c-secret, supported_models: [c]}
      - {id: disabled, api_key: disabled-secret, enabled: false, supported_models: [b, c]}
model_routes: []
aliases: {}
account_groups:
  - {id: b-group, name: B, provider_credential_ids: [b-account]}
  - {id: c-group, name: C, provider_credential_ids: [c-account]}
  - {id: disabled-group, name: Disabled, provider_credential_ids: [disabled]}
"#,
    )
    .unwrap();
    RouteConfigStore::from_document(config).unwrap()
}

#[test]
fn model_mapping_targets_keep_credential_and_model_paired() {
    let store = mapped_store();
    for (group, model, secret) in [("b-group", "b", "b-secret"), ("c-group", "c", "c-secret")] {
        let candidates = store
            .resolve_candidates_for_account_group(Some("a"), Some(group))
            .unwrap();
        assert_eq!(candidates[0].upstream_model.as_deref(), Some(model));
        assert_eq!(candidates[0].payload.api_key, secret);
    }
    assert!(store
        .resolve_candidates_for_account_group(Some("a"), Some("disabled-group"))
        .unwrap()
        .is_empty());
    for _ in 0..32 {
        let candidate = store.resolve_candidates(Some("a")).remove(0);
        match candidate.upstream_model.as_deref() {
            Some("b") => assert_eq!(candidate.payload.api_key, "b-secret"),
            Some("c") => assert_eq!(candidate.payload.api_key, "c-secret"),
            other => panic!("unexpected mapping {other:?}"),
        }
        // A constructed candidate owns its choice; copying/sending it cannot resample.
        assert_eq!(candidate.clone().upstream_model, candidate.upstream_model);
    }
    assert!(store.list_models().iter().any(|model| model.id == "a"));
    assert_eq!(
        store.resolve_candidates(Some("old"))[0]
            .upstream_model
            .as_deref(),
        Some("b")
    );
    assert_eq!(
        store.resolve_candidates(Some("unmapped"))[0]
            .upstream_model
            .as_deref(),
        Some("unmapped")
    );
}

#[test]
fn model_mapping_targets_reject_invalid_and_preserve_old_documents() {
    for targets in [
        json!([]),
        json!(["b", "b"]),
        json!(["*"]),
        json!([" "]),
        json!(vec!["b"; 33]),
    ] {
        let config = json!({"providers":[{"id":"p","preset":"openai","base_url":"http://127.0.0.1:1","model_map_targets":{"a":targets}}],"model_routes":[]});
        assert!(serde_json::from_value::<RouteConfigYaml>(config).is_err());
    }
    let legacy: RouteConfigYaml = serde_json::from_value(
        json!({"providers":[{"id":"p","preset":"openai","base_url":"http://127.0.0.1:1","model_map":{"a":"b"}}],"model_routes":[]}),
    )
    .unwrap();
    assert!(legacy.providers[0].model_map_targets.is_empty());
    let encoded = serde_json::to_value(legacy).unwrap();
    assert_eq!(encoded["providers"][0]["model_map"]["a"], "b");
    assert!(encoded["providers"][0].get("model_map_targets").is_none());
}

#[test]
fn model_mapping_targets_do_not_fallback_or_advertise_unsupported_targets() {
    let store = RouteConfigStore::from_document(
        serde_json::from_value(json!({
        "providers":[{"id":"p","preset":"openai","base_url":"http://127.0.0.1:1","model_map_targets":{"a":["b","c"]},
            "credentials":[{"id":"only-d","api_key":"key","supported_models":["d"]}]}],
        "model_routes":[{"pattern":"a","provider_ids":["p"]}],"aliases":{}}))
        .unwrap(),
    )
    .unwrap();
    assert!(store.resolve_candidates(Some("a")).is_empty());
    assert!(!store.list_models().iter().any(|model| model.id == "a"));
}
