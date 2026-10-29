//! Provider-only creation must never manufacture a routable default credential.
use super::*;

fn document(explicit: bool) -> RouteConfigYaml {
    serde_json::from_value(json!({
        "providers": [{"id":"empty", "preset":"openai", "base_url":"https://example.test/v1",
            "explicit_credentials_only":explicit, "supported_models":["fixture-model"],
            "scheduled_probe_enabled":true, "credentials":[]}],
        "model_routes":[{"pattern":"fixture-model", "provider_ids":["empty"]}]
    }))
    .unwrap()
}

#[test]
fn explicit_empty_pool_has_no_default_routes_inventory_or_probes() {
    let raw = document(true);
    assert_eq!(
        serde_json::to_value(&raw).unwrap()["providers"][0]["explicit_credentials_only"],
        true
    );
    let store = make_store_from_yaml(&serde_yaml::to_string(&raw).unwrap());
    let snapshot = store.snapshot();
    let inventory = snapshot.account_group_inventory();
    assert_eq!(inventory.providers.len(), 1);
    assert!(inventory.accounts.is_empty());
    assert!(snapshot.list_models().is_empty());
    assert!(snapshot
        .resolve_candidates(Some("fixture-model"))
        .is_empty());
    assert!(snapshot.resolve_candidates(None).is_empty());
    assert!(snapshot
        .credential_probe_targets_for_provider("empty")
        .unwrap()
        .is_empty());
    assert!(snapshot
        .select_credential_probe_target("empty::default")
        .is_none());
    assert!(snapshot.scheduled_credential_probe_targets().is_empty());
    assert_eq!(
        snapshot.credential_dispatch_allowed("empty", None),
        Some(false)
    );
    assert!(select_credential(&snapshot.get_providers()[0], None, None, None).is_none());
}

#[test]
fn legacy_default_and_first_explicit_credential_remain_compatible() {
    let legacy = compile_yaml(document(false)).unwrap();
    assert!(select_credential(&legacy.providers[0], None, None, None).is_some());
    let mut raw = document(true);
    raw.providers[0]
        .credentials
        .push(serde_json::from_value(json!({"id":"first", "api_key":"fixture"})).unwrap());
    let compiled = compile_yaml(raw.clone()).unwrap();
    assert_eq!(
        select_credential(&compiled.providers[0], None, None, None)
            .unwrap()
            .credential_id
            .as_deref(),
        Some("first")
    );
    raw.providers[0].credentials.clear();
    let compiled = compile_yaml(raw).unwrap();
    assert!(select_credential(&compiled.providers[0], None, None, None).is_none());
}
