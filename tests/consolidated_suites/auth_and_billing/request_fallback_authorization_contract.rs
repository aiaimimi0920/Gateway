//! Explicit route provenance, never authorization inferred from automatic candidates.
use neuro_gateway::routing::config::{RouteConfigStore, RouteConfigYaml};
use serde_json::{json, Value};

fn document(routes: Value) -> RouteConfigYaml {
    serde_json::from_value(json!({
        "providers": [
            {"id":"proxy-a","base_url":"http://127.0.0.1:1","api_key":"fixture-token","supported_models":["upstream-model"]},
            {"id":"proxy-b","base_url":"http://127.0.0.1:1","api_key":"fixture-token","supported_models":["upstream-model"]}
        ],
        "model_routes": routes, "aliases":{"caller-alias":"upstream-model"}
    })).unwrap()
}

#[test]
fn automatic_support_or_all_provider_fallback_never_authorizes_replay() {
    let store = RouteConfigStore::from_document(document(json!([]))).unwrap();
    for model in [None, Some("caller-alias"), Some("unknown-model")] {
        let resolution = store
            .snapshot()
            .resolve_candidates_with_authorization_for_account_group(model, None)
            .unwrap();
        assert_eq!(resolution.candidates.len(), 2);
        assert!(resolution.explicit_provider_ids.is_none());
    }
}

#[test]
fn enabled_matching_rules_preserve_alias_priority_dedup_and_exact_ids() {
    let store = RouteConfigStore::from_document(document(json!([
        {"pattern":"upstream-*","provider_ids":["proxy-a","proxy-b"],"priority":1},
        {"pattern":"upstream-model","provider_ids":["proxy-b"],"priority":2},
        {"pattern":"unrelated-model","provider_ids":["proxy-a"],"priority":100}
    ])))
    .unwrap();
    let snapshot = store.snapshot();
    let resolution = snapshot
        .resolve_candidates_with_authorization_for_account_group(Some("caller-alias"), None)
        .unwrap();
    assert_eq!(
        resolution.explicit_provider_ids,
        Some(vec!["proxy-b".into(), "proxy-a".into()])
    );
    assert_eq!(
        resolution
            .candidates
            .iter()
            .map(|candidate| candidate.provider_account_id.as_str())
            .collect::<Vec<_>>(),
        vec!["proxy-b", "proxy-a"]
    );
    // A later route install cannot change the authority of the pinned resolution.
    store.replace_document(document(json!([]))).unwrap();
    assert!(store
        .snapshot()
        .resolve_candidates_with_authorization_for_account_group(Some("caller-alias"), None)
        .unwrap()
        .explicit_provider_ids
        .is_none());
    assert_eq!(
        snapshot
            .resolve_candidates_with_authorization_for_account_group(Some("caller-alias"), None)
            .unwrap()
            .explicit_provider_ids
            .unwrap()
            .len(),
        2
    );
}

#[test]
fn disabled_matches_do_not_fall_back_to_automatic_authority() {
    let store = RouteConfigStore::from_document(document(json!([
        {"pattern":"upstream-*","provider_ids":["proxy-a","proxy-b"],"enabled":false}
    ])))
    .unwrap();
    let resolution = store
        .snapshot()
        .resolve_candidates_with_authorization_for_account_group(Some("caller-alias"), None)
        .unwrap();
    assert!(resolution.candidates.is_empty());
    assert_eq!(resolution.explicit_provider_ids, Some(vec![]));
}

#[test]
fn explicit_rules_cannot_bypass_an_account_group_filter() {
    let mut doc =
        document(json!([{"pattern":"upstream-model","provider_ids":["proxy-a","proxy-b"]}]));
    doc.account_groups = serde_json::from_value(json!([
        {"id":"restricted-group","name":"fixture group","enabled":true,"provider_credential_ids":["proxy-a::default"]}
    ])).unwrap();
    let store = RouteConfigStore::from_document(doc).unwrap();
    let resolution = store
        .snapshot()
        .resolve_candidates_with_authorization_for_account_group(
            Some("caller-alias"),
            Some("restricted-group"),
        )
        .unwrap();
    assert_eq!(resolution.candidates.len(), 1);
    assert_eq!(resolution.candidates[0].provider_account_id, "proxy-a");
}
