//! Expanding the retry queue must preserve the primary pool's round-robin and authorization.
use super::*;

#[test]
fn bounded_backups_do_not_consume_primary_round_robin() {
    for size in [2, 3, 6, 8] {
        for mapped in [false, true] {
            let credentials: Vec<_> = (0..size)
                .map(|i| {
                    serde_json::json!({
                        "id": format!("c{i}"), "api_key":"fixture", "supported_models":["upstream"]
                    })
                })
                .collect();
            let mut provider = serde_json::json!({"id":"site","preset":"openai",
                "base_url":"http://127.0.0.1:1","supported_models":["alias"],"credentials":credentials});
            if mapped {
                provider["model_map_targets"] = serde_json::json!({"alias":["upstream"]});
            } else {
                provider["model_map"] = serde_json::json!({"alias":"upstream"});
            }
            let store = RouteConfigStore::from_document(
                serde_json::from_value(serde_json::json!({
                    "providers":[provider]
                }))
                .unwrap(),
            )
            .unwrap();
            let snapshot = store.snapshot();
            let constraint = snapshot.account_group_constraint(None).unwrap();
            for i in 0..size * 2 {
                let result = snapshot.resolve_candidates_admitted(
                    Some("alias"),
                    &constraint,
                    &|_, _, _| true,
                    false,
                );
                assert_eq!(result.candidates.len(), size.min(6));
                assert_eq!(
                    result.candidates[0].payload.credential_id.as_deref(),
                    Some(format!("c{}", i % size).as_str())
                );
                assert_eq!(
                    result
                        .candidates
                        .iter()
                        .filter_map(|c| c.payload.credential_id.as_ref())
                        .collect::<HashSet<_>>()
                        .len(),
                    size.min(6)
                );
                assert!(result.candidates[1..]
                    .iter()
                    .all(|c| c.priority == i32::MIN));
            }
        }
    }
}

#[test]
fn request_group_alone_cannot_expand_explicit_provider_fallback() {
    let store = RouteConfigStore::from_document(serde_json::from_value(serde_json::json!({
        "providers":[{"id":"a","preset":"openai","base_url":"http://127.0.0.1:1","api_key":"fixture","supported_models":["m"]},
            {"id":"b","preset":"openai","base_url":"http://127.0.0.1:1","api_key":"fixture","supported_models":["m"]}],
        "account_groups":[{"id":"g","name":"G","provider_credential_ids":["a::default","b::default"]}],
        "model_routes":[{"pattern":"m","provider_ids":["a"]}]
    })).unwrap()).unwrap();
    let snapshot = store.snapshot();
    let constraint = snapshot.account_group_constraint(Some("g")).unwrap();
    let without =
        snapshot.resolve_candidates_admitted(Some("m"), &constraint, &|_, _, _| true, false);
    assert_eq!(without.candidates.len(), 1);
    let entitled =
        snapshot.resolve_candidates_admitted(Some("m"), &constraint, &|_, _, _| true, true);
    assert_eq!(entitled.candidates.len(), 2);
    assert_eq!(
        entitled.explicit_provider_ids,
        Some(vec!["a".into(), "b".into()])
    );
}
