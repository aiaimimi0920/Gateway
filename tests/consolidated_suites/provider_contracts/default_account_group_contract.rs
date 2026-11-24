//! Fallback membership is shared by inventory and authorization, not a storage migration.
use neuro_gateway::{
    console::document::validate_route_document,
    routing::config::{RouteConfigStore, RouteConfigYaml},
};
use serde_json::json;

fn document() -> RouteConfigYaml {
    serde_json::from_value(json!({"providers":[
        {"id":"pool","adapter":"openai_compatible","base_url":"https://fixture.invalid/v1",
         "supported_models":["fixture"],"credentials":[
            {"id":"chosen-account","api_key":"synthetic-1"},
            {"id":"free-account","api_key":"synthetic-2"},
            {"id":"disabled-account","api_key":"synthetic-3","enabled":false}
         ]},
        {"id":"legacy","adapter":"openai_compatible","base_url":"https://fixture.invalid/v1",
         "api_key":"synthetic-4","supported_models":["fixture"]}
    ],"account_groups":[{"id":"chosen","name":"chosen","enabled":false,
        "provider_credential_ids":["chosen-account"]}]}))
    .unwrap()
}

#[test]
fn default_membership_covers_unassigned_accounts_and_preserves_revision_document() {
    let document = document();
    let original = serde_json::to_value(&document).unwrap();
    let validated = validate_route_document(document.clone()).unwrap();
    assert_eq!(
        serde_json::to_value(validated.document()).unwrap(),
        original
    );
    let store = RouteConfigStore::from_document(document).unwrap();
    let snapshot = store.snapshot();
    let inventory = snapshot.account_group_inventory();
    let default = inventory
        .account_groups
        .iter()
        .find(|g| g.id == "default")
        .unwrap();
    assert_eq!(
        default.provider_credential_ids,
        ["free-account", "disabled-account", "legacy::default"]
    );
    assert_eq!(default.billing_multiplier, 1.0);
    assert!(inventory.accounts.iter().all(|a| !a.group_ids.is_empty()));
    let constraint = snapshot.access_key_group_constraint(&["default".into()]);
    let candidates = snapshot
        .resolve_candidates_for_account_group(Some("fixture"), Some("default"))
        .unwrap();
    assert!(!candidates.is_empty());
    assert!(candidates.iter().all(|c| constraint.allows_candidate(c)));
    assert!(candidates
        .iter()
        .all(|c| c.provider_credential_id.as_deref() != Some("chosen-account")));
    assert!(snapshot
        .resolve_candidates_for_account_group(Some("fixture"), Some("chosen"))
        .is_err());
}

#[test]
fn default_membership_is_derived_and_disabled_default_is_not_bypassed() {
    let mut document = document();
    document.account_groups.push(serde_json::from_value(json!({"id":"default","name":"default",
        "enabled":false,"billing_multiplier":2,"provider_credential_ids":["chosen-account","deleted"]})).unwrap());
    let store = RouteConfigStore::from_document({
        // A stale reference is invalid in a strict document; explicit membership
        // in another group must nevertheless override the saved default list.
        document.account_groups[1].provider_credential_ids = vec!["chosen-account".into()];
        document
    })
    .unwrap();
    let snapshot = store.snapshot();
    let inventory = snapshot.account_group_inventory();
    let group = inventory
        .account_groups
        .iter()
        .find(|g| g.id == "default")
        .unwrap();
    assert!(!group.enabled);
    assert_eq!(group.billing_multiplier, 2.0);
    assert!(!group
        .provider_credential_ids
        .contains(&"chosen-account".into()));
    assert!(snapshot
        .resolve_candidates_for_account_group(Some("fixture"), Some("default"))
        .is_err());
    let constraint = snapshot.access_key_group_constraint(&["default".into()]);
    assert!(constraint
        .filter_candidates(snapshot.resolve_candidates(Some("fixture")))
        .is_empty());
}

#[test]
fn whitespace_default_id_uses_the_inventory_identity_for_key_authorization() {
    let mut document = document();
    document.account_groups.push(
        serde_json::from_value(json!({
            "id":" default ", "name":"default", "provider_credential_ids":[]
        }))
        .unwrap(),
    );
    let store = RouteConfigStore::from_document(document).unwrap();
    let snapshot = store.snapshot();
    let constraint = snapshot.access_key_group_constraint(&["default".into()]);
    let candidates = snapshot
        .resolve_candidates_for_account_group(Some("fixture"), Some("default"))
        .unwrap();
    assert!(!candidates.is_empty());
    assert!(candidates.iter().all(|c| constraint.allows_candidate(c)));
}
