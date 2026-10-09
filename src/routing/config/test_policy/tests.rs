use super::*;
use serde_json::json;

fn policy(enabled: bool, model: &str) -> serde_json::Value {
    json!({"automaticEnabled":enabled,"intervalMinutes":15,"modelSelection":"selected","models":[model],
        "cases":[{"id":"ok","name":"OK","prompt":"Reply OK","expectedAnswer":"OK","difficulty":1,"enabled":true}]})
}

#[test]
fn pool_subpool_account_inheritance_and_explicit_disabling_do_not_rewrite_children() {
    let document: RouteConfigYaml = serde_json::from_value(json!({"providers":[{"id":"pool","base_url":"https://example.invalid/v1",
        "test_policy":policy(true,"pool-model"), "subpool_test_policies":{"free":policy(true,"group-model")},
        "credentials":[{"id":"one","api_key":"one-secret","credential_identity_category_id":"free"},
            {"id":"two","api_key":"two-secret","credential_identity_category_id":"free","test_policy":policy(false,"account-model")},
            {"id":"three","api_key":"three-secret"}]}]})).unwrap();
    let store = RouteConfigStore::from_document(document.clone()).unwrap();
    let snapshot = store.snapshot();
    for (id, source, model) in [
        ("one", "subpool:free", "group-model"),
        ("two", "account", "account-model"),
        ("three", "pool", "pool-model"),
    ] {
        let target = snapshot.select_credential_probe_target(id).unwrap();
        let (resolved, actual) = snapshot.credential_test_policy(&target).unwrap();
        assert_eq!(actual, source);
        assert_eq!(resolved.models, vec![model]);
    }
    let scheduled: Vec<_> = snapshot
        .scheduled_credential_probe_targets()
        .into_iter()
        .map(|target| target.target.credential_id)
        .collect();
    assert_eq!(scheduled, vec!["one", "three"]);
    let mut changed = document;
    changed.providers[0].test_policy.as_mut().unwrap().models = vec!["new-pool-model".into()];
    let round_trip = serde_json::to_value(changed).unwrap();
    assert_eq!(
        round_trip["providers"][0]["credentials"][1]["test_policy"]["models"],
        json!(["account-model"])
    );
    assert_eq!(
        round_trip["providers"][0]["credentials"][0]["api_key"],
        "one-secret"
    );
}

#[test]
fn old_documents_round_trip_without_added_policy_fields_and_default_account_can_override() {
    let old: RouteConfigYaml = serde_json::from_value(json!({"providers":[{"id":"legacy","base_url":"https://example.invalid/v1","api_key":"secret","scheduled_probe_enabled":true}]})).unwrap();
    let value = serde_json::to_value(&old).unwrap();
    assert!(value["providers"][0].get("test_policy").is_none());
    assert!(value["providers"][0].get("subpool_test_policies").is_none());
    assert!(value["providers"][0]
        .get("default_account_test_policy")
        .is_none());
    assert_eq!(
        RouteConfigStore::from_document(old)
            .unwrap()
            .snapshot()
            .scheduled_credential_probe_targets()
            .len(),
        1
    );
    let current = serde_json::from_value(json!({"providers":[{"id":"legacy","base_url":"https://example.invalid/v1","api_key":"secret",
        "test_policy":policy(true,"parent"),"default_account_test_policy":policy(false,"child")}]})).unwrap();
    let store = RouteConfigStore::from_document(current).unwrap();
    let snapshot = store.snapshot();
    assert!(snapshot.scheduled_credential_probe_targets().is_empty());
    let target = snapshot
        .select_credential_probe_target("legacy::default")
        .unwrap();
    assert_eq!(
        snapshot.credential_test_policy(&target).unwrap().1,
        "account"
    );
}
