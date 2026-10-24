use super::*;

fn store() -> RouteConfigStore {
    let store = RouteConfigStore::new();
    store
        .replace_document(
            serde_json::from_value(serde_json::json!({
                "providers":[{"id":"site","preset":"openai","base_url":"https://example.invalid",
                    "supported_models":["alias"],"model_map_targets":{"alias":["bad","good"]},
                    "credentials":[{"id":"plus","api_key":"fixture","supported_models":["bad"]},
                        {"id":"pro","api_key":"fixture","supported_models":["good"]}]}],
                "account_groups":[{"id":"plus","name":"Plus","provider_credential_ids":["plus"]},
                    {"id":"pro","name":"Pro","provider_credential_ids":["pro"]}]
            }))
            .unwrap(),
        )
        .unwrap();
    store
}

#[test]
fn catalogue_checks_never_consume_round_robin_or_choose_a_rejected_mapping() {
    let store = store();
    let snapshot = store.snapshot();
    let allow = |_: &str, _: &str, model: &str| model != "bad";
    for _ in 0..32 {
        assert!(snapshot.has_admitted_candidate("alias", None, None, &allow));
        assert!(!snapshot.has_admitted_candidate("alias", None, Some(&["other".into()]), &allow));
    }
    assert_eq!(
        snapshot.compiled().providers[0]
            .credential_counter
            .load(Ordering::Relaxed),
        0
    );
    let plus = snapshot.account_group_constraint(Some("plus")).unwrap();
    assert!(!snapshot.has_admitted_candidate("alias", Some(&plus), None, &allow));
    assert!(snapshot
        .resolve_candidates_admitted(Some("alias"), &plus, &allow, false)
        .candidates
        .is_empty());
    let all = snapshot.account_group_constraint(None).unwrap();
    for _ in 0..32 {
        let resolved = snapshot.resolve_candidates_admitted(Some("alias"), &all, &allow, false);
        assert_eq!(resolved.candidates.len(), 1);
        assert_eq!(
            resolved.candidates[0].upstream_model.as_deref(),
            Some("good")
        );
        assert_eq!(
            resolved.candidates[0].payload.credential_id.as_deref(),
            Some("pro")
        );
    }
}

#[test]
fn dynamic_recovery_never_reenables_manually_disabled_credentials_or_rules() {
    let store = store();
    let mut document = store.snapshot().document().clone();
    for credential in &mut document.providers[0].credentials {
        credential.enabled = Some(false);
    }
    store.replace_document(document).unwrap();
    let snapshot = store.snapshot();
    let all = snapshot.account_group_constraint(None).unwrap();
    assert!(!snapshot.has_admitted_candidate("alias", None, None, &|_, _, _| true));
    assert!(snapshot
        .resolve_candidates_admitted(Some("alias"), &all, &|_, _, _| true, false)
        .candidates
        .is_empty());
}
