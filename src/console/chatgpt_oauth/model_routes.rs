//! OAuth model discovery must register routes just like API-key discovery.
use crate::routing::config::{ModelRoute, RouteConfigYaml};

pub(super) fn register(document: &mut RouteConfigYaml, provider: &str, models: &[String]) {
    for model in models {
        // Preserve explicit policy, including disabled routes and other providers.
        if let Some(route) = document
            .model_routes
            .iter_mut()
            .find(|r| r.pattern == *model)
        {
            if !route.provider_ids.iter().any(|id| id == provider) {
                route.provider_ids.push(provider.to_owned());
            }
        } else {
            document.model_routes.push(ModelRoute {
                pattern: model.clone(),
                provider_ids: vec![provider.to_owned()],
                priority: 10,
                enabled: true,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::routing::config::RouteConfigStore;

    fn document() -> RouteConfigYaml {
        serde_json::from_value(serde_json::json!({
            "providers": [
                {"id":"relay", "base_url":"https://example.invalid/v1", "api_key":"fixture", "supported_models":["gpt-6.1-sol"]},
                {"id":"chatgpt", "preset":"chatgpt-codex-oauth-official-api", "base_url":"https://chatgpt.com/backend-api/codex",
                 "supported_models":["gpt-6.1-sol"], "credentials":[{"id":"oauth-test", "api_key":"fixture", "headers":{"Chatgpt-Account-Id":"fixture"},
                 "supported_models":["gpt-6.1-sol"], "credential_identity_category_id":"pro20x"}]}
            ],
            "model_routes":[{"pattern":"gpt-6.1-sol", "provider_ids":["relay"], "priority":99}]
        })).unwrap()
    }

    #[test]
    fn oauth_import_adds_missing_candidate_without_replacing_existing_policy() {
        let mut doc = document();
        let before = RouteConfigStore::from_document(doc.clone()).unwrap();
        assert_eq!(before.resolve_candidates(Some("gpt-6.1-sol")).len(), 1);
        register(&mut doc, "chatgpt", &["gpt-6.1-sol".into()]);
        register(&mut doc, "chatgpt", &["gpt-6.1-sol".into()]);
        assert_eq!(doc.model_routes[0].provider_ids, ["relay", "chatgpt"]);
        assert_eq!(doc.model_routes[0].priority, 99);
        let after = RouteConfigStore::from_document(doc).unwrap();
        assert_eq!(after.resolve_candidates(Some("gpt-6.1-sol")).len(), 2);
    }

    #[test]
    fn importing_does_not_enable_disabled_routes() {
        let mut doc = document();
        doc.model_routes[0].enabled = false;
        register(&mut doc, "chatgpt", &["gpt-6.1-sol".into()]);
        assert!(!doc.model_routes[0].enabled);
        assert!(RouteConfigStore::from_document(doc)
            .unwrap()
            .resolve_candidates(Some("gpt-6.1-sol"))
            .is_empty());
    }

    #[test]
    fn newly_discovered_model_gets_one_exact_route() {
        let mut doc = document();
        register(
            &mut doc,
            "chatgpt",
            &["new-model".into(), "new-model".into()],
        );
        assert_eq!(doc.model_routes.len(), 2);
        assert_eq!(doc.model_routes[1].pattern, "new-model");
        assert_eq!(doc.model_routes[1].provider_ids, ["chatgpt"]);
        assert_eq!(doc.model_routes[1].priority, 10);
        assert!(doc.model_routes[1].enabled);
    }
}
