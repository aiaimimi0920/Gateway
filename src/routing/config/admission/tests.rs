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

#[test]
fn vercel_catalog_applies_the_actual_account_tier_and_mapped_target() {
    let store = RouteConfigStore::new();
    let zero = "recursive/flint-v1.0";
    let credit = "alibaba/qwen-3-14b";
    let retired = "reflection/beam-free";
    store.replace_document(serde_json::from_value(serde_json::json!({
        "providers": [
            {"id":"vercel", "preset":"vercel-ai-gateway", "base_url":"https://ai-gateway.vercel.sh/v1",
             "supported_models":[zero, credit, retired],
             "model_map":{"old-single":retired, "credit-single":credit},
             "model_map_targets":{"old-multi":[retired], "mixed-targets":[retired,zero]},
             "credentials":[
                 {"id":"free", "api_key":"fixture", "supported_models":[zero, credit, retired]},
                 {"id":"credit", "api_key":"other-fixture", "auth_mode":"vercel_monthly_credit_key",
                  "model_selection":{"manual":[credit,retired],"fetched":[]}}
             ]},
            {"id":"other","preset":"openai","base_url":"https://example.invalid","api_key":"fixture","supported_models":["shared"]}
        ],
        "aliases":{"old-alias":"old-single"},
        "model_routes":[
            {"pattern":"*","provider_ids":["vercel"]},
            {"pattern":"shared","priority":20,"provider_ids":["vercel","other"]}
        ],
        "account_groups":[{"id":"free-only","name":"Free","provider_credential_ids":["free"]}]
    })).unwrap()).unwrap();
    let snapshot = store.snapshot();
    let all = snapshot.account_group_constraint(None).unwrap();
    let free_only = snapshot
        .account_group_constraint(Some("free-only"))
        .unwrap();
    let allow = |_: &str, _: &str, _: &str| true;
    let listed: HashSet<_> = snapshot
        .list_models()
        .into_iter()
        .map(|row| row.id)
        .collect();
    for model in [retired, "old-single", "old-multi", "old-alias"] {
        assert!(!listed.contains(model), "{model}");
        assert!(
            !snapshot.has_admitted_candidate(model, None, None, &allow),
            "{model}"
        );
        assert!(
            snapshot.resolve_candidates(Some(model)).is_empty(),
            "{model}"
        );
    }
    for model in [credit, "credit-single"] {
        assert!(listed.contains(model), "{model}");
        assert!(
            !snapshot.has_admitted_candidate(model, Some(&free_only), None, &allow),
            "{model}"
        );
        let resolved = snapshot.resolve_candidates_admitted(Some(model), &all, &allow, false);
        assert_eq!(resolved.candidates.len(), 1);
        assert_eq!(
            resolved.candidates[0].payload.credential_id.as_deref(),
            Some("credit")
        );
    }
    assert!(listed.contains("mixed-targets"));
    assert_eq!(
        snapshot.resolve_candidates(Some("mixed-targets"))[0]
            .upstream_model
            .as_deref(),
        Some(zero)
    );
    assert!(listed.contains("shared"));
    assert_eq!(
        snapshot.resolve_candidates(Some("shared"))[0].provider_account_id,
        "other"
    );
}
