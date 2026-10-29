//! Editable sources survive reload, including overlap and intentional empty pools.
use super::*;

fn document() -> RouteConfigYaml {
    serde_json::from_value(json!({"providers":[{"id":"poe","preset":"poe-openai",
        "base_url":"https://api.poe.com/v1", "explicit_credentials_only":true,
        "supported_models":["manual","removed","fetched"],"credentials":[{
            "id":"account","api_key":"fixture-key","account_name":"Legacy", "enabled":true,
            "supported_models":["stale"],
            "model_catalog":{"sourceUrl":"https://api.poe.com/v1/models",
                "checkedAt":"2026-10-26T04:00:00Z","models":["fetched","removed"]},
            "model_selection":{"fetched":["fetched"],"manual":["manual","fetched"]}
        }]}],"model_routes":[{"pattern":"manual","provider_ids":["poe"],"priority":42}]}))
    .unwrap()
}

#[test]
fn model_selection_survives_repeat_reload_and_controls_candidates_without_verification() {
    let mut doc = document();
    for _ in 0..2 {
        let encoded = serde_yaml::to_string(&doc).unwrap();
        doc = serde_yaml::from_str(&encoded).unwrap();
        let store = make_store_from_yaml(&encoded);
        for model in ["manual", "fetched"] {
            let candidates = store.resolve_candidates(Some(model));
            assert_eq!(candidates.len(), 1);
            assert_eq!(candidates[0].payload.api_key, "fixture-key");
            assert_eq!(
                candidates[0].payload.account_name.as_deref(),
                Some("Legacy")
            );
        }
        for excluded in ["removed", "stale", "unlisted"] {
            assert!(store.resolve_candidates(Some(excluded)).is_empty());
            assert!(store.list_models().iter().all(|model| model.id != excluded));
        }
        let compiled = compile_provider(doc.providers[0].clone()).unwrap();
        assert_eq!(
            compiled.credential_pool[0].supported_models,
            ["manual", "fetched"]
        );
        assert!(compiled.credential_pool[0].discovery.is_none());
        assert_eq!(doc.model_routes[0].priority, 42);
        assert_eq!(
            doc.providers[0].credentials[0]
                .model_selection
                .as_ref()
                .unwrap()
                .manual,
            ["manual", "fetched"]
        );
    }
}

#[test]
fn model_selection_empty_is_not_legacy_inheritance_or_an_enabled_route() {
    let mut doc = document();
    doc.providers[0].credentials[0].model_selection = Some(ModelSelection {
        fetched: vec![],
        manual: vec![],
    });
    let store = make_store_from_yaml(&serde_yaml::to_string(&doc).unwrap());
    assert!(store.resolve_candidates(Some("manual")).is_empty());
    assert!(store.resolve_candidates(None).is_empty());
    assert!(store.account_group_inventory().accounts[0]
        .supported_models
        .is_empty());
    assert!(store.list_models().is_empty());
    // Old documents still use their existing explicit list and catalog union.
    doc.providers[0].credentials[0].model_selection = None;
    let legacy = make_store_from_yaml(&serde_yaml::to_string(&doc).unwrap());
    assert_eq!(legacy.resolve_candidates(Some("removed")).len(), 1);
    doc.providers[0].credentials[0].model_selection = Some(ModelSelection {
        fetched: vec![],
        manual: vec!["manual".into()],
    });
    doc.model_routes[0].enabled = false;
    assert!(make_store_from_yaml(&serde_yaml::to_string(&doc).unwrap())
        .resolve_candidates(Some("manual"))
        .is_empty());
}

#[test]
fn model_selection_validates_bounded_exact_ids_and_deduplicates_sources() {
    let selection = |models| ModelSelection {
        fetched: vec![],
        manual: models,
    };
    for invalid in ["", " padded", "bad\nmodel", "gpt-*", "model?"] {
        assert!(selection(vec![invalid.into()]).models().is_err());
    }
    assert!(selection(vec!["模".repeat(86)]).models().is_err());
    assert!(selection(vec!["a".into(); 2049]).models().is_err());
    assert_eq!(
        selection(vec!["a".into(), "a".into()]).models().unwrap(),
        ["a"]
    );
    let mut doc = document();
    doc.providers[0].credentials[0].model_selection = Some(selection(vec!["*".into()]));
    assert!(compile_provider(doc.providers.remove(0)).is_err());
}

#[test]
fn model_selection_preserves_alias_and_multi_target_routing() {
    let mut doc = document();
    doc.providers[0]
        .model_map
        .insert("alias-one".into(), "fetched".into());
    doc.providers[0]
        .model_map_targets
        .insert("alias-many".into(), vec!["removed".into(), "manual".into()]);
    let store = make_store_from_yaml(&serde_yaml::to_string(&doc).unwrap());
    for alias in ["alias-one", "alias-many"] {
        assert!(store.list_models().iter().any(|model| model.id == alias));
        assert_eq!(store.resolve_candidates(Some(alias)).len(), 1);
    }
    assert_eq!(
        store.resolve_candidates(Some("alias-many"))[0]
            .upstream_model
            .as_deref(),
        Some("manual")
    );
}

#[test]
fn model_selection_background_discovery_preserves_manual_models_and_evidence() {
    use crate::provider_discovery::{binding, job, CredentialDiscovery, DiscoveredProtocol};
    let mut doc: RouteConfigYaml = serde_json::from_value(json!({"providers":[{"id":"generic",
        "base_url":"https://fixture.test", "adapter":"openai_compatible", "credentials":[{
        "id":"account","api_key":"fixture-key","model_selection":{"fetched":["old"],"manual":["manual","shared"]},
        "supported_models":["old","manual","shared"], "discovery_job":{"id":"d561681c-6ce0-42ae-918f-cce575fc003c","status":"pending"}
    }]}],"model_routes":[]})).unwrap();
    let job = doc.providers[0].credentials[0]
        .discovery_job
        .clone()
        .unwrap();
    let discovery = CredentialDiscovery {
        source_url: "https://fixture.test".into(),
        api_base: "https://fixture.test/v1".into(),
        protocol: DiscoveredProtocol::ChatCompletions,
        models: vec!["fetched".into()],
        verified_models: vec!["fetched".into()],
        protocols: vec![],
        probes: vec![],
        checked_at: "2026-10-26T04:00:00Z".into(),
        binding: binding("https://fixture.test", "fixture-key"),
    };
    assert!(job::merge(
        &mut doc,
        "generic",
        "account",
        &job,
        Some(&discovery)
    ));
    let cred = &doc.providers[0].credentials[0];
    assert_eq!(cred.supported_models, ["manual", "shared", "fetched"]);
    assert_eq!(
        cred.discovery.as_ref().unwrap().verified_models,
        ["fetched"]
    );
    let store = make_store_from_yaml(&serde_yaml::to_string(&doc).unwrap());
    assert_eq!(store.resolve_candidates(Some("manual")).len(), 1);
    assert!(store.resolve_candidates(Some("old")).is_empty());
    assert_eq!(store.resolve_candidates(Some("fetched")).len(), 1);
}
