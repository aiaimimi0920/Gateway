//! Metadata survives reload and feeds runtime pools without fabricating verification.
use super::*;

fn catalog_document(preset: &str, base: &str, source: &str) -> RouteConfigYaml {
    serde_json::from_value(json!({"providers":[{"id":"official","preset":preset,"base_url":base,
        "explicit_credentials_only":true,"supported_models":["manual"],"credentials":[
            {"id":"enabled","api_key":"fixture-kept","supported_models":["manual"],
             "model_catalog":{"sourceUrl":source,"checkedAt":"2026-10-26T04:00:00Z","models":["vendor/one","vendor/two"]}},
            {"id":"disabled","api_key":"fixture-disabled","enabled":false,"supported_models":[],
             "model_catalog":{"sourceUrl":source,"checkedAt":"2026-10-26T04:00:00Z","models":["vendor/disabled"]}}
        ]}],"model_routes":[]})).unwrap()
}

#[test]
fn catalogs_project_to_credentials_inventory_and_routes_after_reload() {
    for (preset, base, source) in [
        (
            "nvidia-openai",
            "https://integrate.api.nvidia.com",
            crate::provider_discovery::model_catalog::NVIDIA_SOURCE,
        ),
        (
            "openrouter-openai",
            "https://openrouter.ai/api/v1",
            crate::provider_discovery::model_catalog::OPENROUTER_SOURCE,
        ),
        (
            "poe-openai",
            "https://api.poe.com/v1",
            crate::provider_discovery::model_catalog::POE_SOURCE,
        ),
        (
            "groq-openai",
            "https://api.groq.com/openai/v1",
            crate::provider_discovery::model_catalog::GROQ_SOURCE,
        ),
        (
            "cohere-chat",
            "https://api.cohere.com",
            crate::provider_discovery::model_catalog::COHERE_SOURCE,
        ),
    ] {
        let doc = catalog_document(preset, base, source);
        let original = serde_yaml::to_string(&doc).unwrap();
        for yaml in [
            &original,
            &serde_yaml::to_string(&serde_yaml::from_str::<RouteConfigYaml>(&original).unwrap())
                .unwrap(),
        ] {
            let store = make_store_from_yaml(yaml);
            let snapshot = store.snapshot();
            for model in ["manual", "vendor/one", "vendor/two"] {
                assert!(
                    store.list_models().iter().any(|entry| entry.id == model),
                    "{preset}: {model}"
                );
                let candidates = store.resolve_candidates(Some(model));
                assert_eq!(candidates.len(), 1);
                assert_eq!(
                    candidates[0].payload.credential_id.as_deref(),
                    Some("enabled")
                );
                assert_eq!(candidates[0].payload.api_key, "fixture-kept");
            }
            // Directory IDs are not entitlements: keep legacy manual fallback, but
            // never select or enable the disabled credential that listed this ID.
            assert!(store
                .resolve_candidates(Some("vendor/disabled"))
                .iter()
                .all(|candidate| candidate.payload.credential_id.as_deref() != Some("disabled")));
            let provider = &snapshot.document().providers[0];
            assert!(provider.credentials[0].discovery.is_none());
            assert_eq!(provider.credentials[0].supported_models, ["manual"]);
            assert_eq!(
                provider.credentials[0]
                    .model_catalog
                    .as_ref()
                    .unwrap()
                    .models
                    .len(),
                2
            );
            assert!(!provider.credentials[1].enabled.unwrap());
        }
    }
}

#[test]
fn catalog_metadata_does_not_bypass_disabled_routes_or_source_validation() {
    let mut doc = catalog_document(
        "openrouter-openai",
        "https://openrouter.ai/api/v1",
        crate::provider_discovery::model_catalog::OPENROUTER_SOURCE,
    );
    doc.model_routes = serde_json::from_value(
        json!([{"pattern":"vendor/two","provider_ids":["official"],"enabled":false}]),
    )
    .unwrap();
    let store = make_store_from_yaml(&serde_yaml::to_string(&doc).unwrap());
    assert!(store.resolve_candidates(Some("vendor/two")).is_empty());
    assert!(store
        .list_models()
        .iter()
        .all(|model| model.id != "vendor/two"));
    doc.providers[0].credentials[0]
        .model_catalog
        .as_mut()
        .unwrap()
        .source_url = crate::provider_discovery::model_catalog::NVIDIA_SOURCE.into();
    assert!(compile_provider(doc.providers.remove(0)).is_err());
}
