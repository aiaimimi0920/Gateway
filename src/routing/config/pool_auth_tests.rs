//! OAuth-only is a provider-pool constraint, never a token-shape or global fallback claim.
use super::*;
use serde_json::json;

fn document(policy: Option<&str>) -> RouteConfigYaml {
    let mut provider = json!({"id":"pool", "adapter":"openai_compatible", "base_url":"https://example.invalid/v1",
    "supported_models":["model"], "credentials":[
        {"id":"api", "api_key":"synthetic-api", "supported_models":["model"]},
        {"id":"oauth", "api_key":"synthetic-access", "refresh_token":"synthetic-refresh",
            "refresh_endpoint":"https://example.invalid/token", "supported_models":["model"]}
    ]});
    if let Some(policy) = policy {
        provider["credential_pool_auth_policy"] = json!(policy);
    }
    serde_json::from_value(json!({"providers":[provider], "model_routes":[], "aliases":{}}))
        .unwrap()
}

#[test]
fn oauth_only_excludes_api_keys_before_credential_round_robin() {
    let store = RouteConfigStore::from_document(document(Some("oauth_only"))).unwrap();
    for _ in 0..64 {
        let candidates = store.snapshot().resolve_candidates(Some("model"));
        assert_eq!(candidates.len(), 1);
        assert_eq!(
            candidates[0].payload.credential_id.as_deref(),
            Some("oauth")
        );
    }
}

#[test]
fn oauth_only_does_not_reuse_legacy_unsupported_model_fallback() {
    let mut doc = document(Some("oauth_only"));
    doc.providers[0].credentials[1].supported_models = vec!["other-model".into()];
    let store = RouteConfigStore::from_document(doc).unwrap();
    assert!(store
        .snapshot()
        .resolve_candidates(Some("model"))
        .is_empty());
    assert!(store.snapshot().list_models().is_empty());
}

#[test]
fn oauth_only_filters_mapping_alternatives_but_explicit_probe_keeps_its_account() {
    let mut doc = document(Some("oauth_only"));
    doc.providers[0].credentials[0].supported_models = vec!["api-target".into()];
    doc.providers[0].credentials[1].supported_models = vec!["oauth-target".into()];
    doc.providers[0].model_map_targets.insert(
        "model".into(),
        vec!["api-target".into(), "oauth-target".into()],
    );
    let store = RouteConfigStore::from_document(doc.clone()).unwrap();
    let route = store.snapshot();
    assert!(route.list_models().iter().any(|model| model.id == "model"));
    for _ in 0..64 {
        let candidates = route.resolve_candidates(Some("model"));
        assert_eq!(
            candidates[0].upstream_model.as_deref(),
            Some("oauth-target")
        );
        assert_eq!(
            candidates[0].payload.credential_id.as_deref(),
            Some("oauth")
        );
    }
    let target = route.select_credential_probe_target("api").unwrap();
    assert_eq!(target.credential_id, "api");
    assert_eq!(
        model_for_probe(&route.compiled.providers[0], "api", "model").as_deref(),
        Some("api-target")
    );
    doc.providers[0].credentials[1].enabled = Some(false);
    let unavailable = RouteConfigStore::from_document(doc).unwrap();
    assert!(unavailable.snapshot().list_models().is_empty());
    assert!(unavailable
        .snapshot()
        .resolve_candidates(Some("model"))
        .is_empty());
}

#[test]
fn policy_round_trip_is_explicit_and_legacy_documents_stay_unchanged() {
    let strict = serde_json::to_value(document(Some("oauth_only"))).unwrap();
    assert_eq!(
        strict["providers"][0]["credential_pool_auth_policy"],
        "oauth_only"
    );
    let legacy = serde_json::to_value(document(None)).unwrap();
    assert!(legacy["providers"][0]
        .get("credential_pool_auth_policy")
        .is_none());
    assert_eq!(serde_json::to_value(document(Some("any"))).unwrap(), legacy);
    let mut invalid = strict;
    invalid["providers"][0]["credential_pool_auth_policy"] = json!("prefer_and_silently_fallback");
    assert!(serde_json::from_value::<RouteConfigYaml>(invalid).is_err());
}

#[test]
fn any_retains_legacy_round_robin_and_unsupported_model_compatibility() {
    let store = RouteConfigStore::from_document(document(None)).unwrap();
    let ids = (0..8)
        .map(|_| {
            store.snapshot().resolve_candidates(Some("unknown"))[0]
                .payload
                .credential_id
                .clone()
                .unwrap()
        })
        .collect::<HashSet<_>>();
    assert_eq!(ids, HashSet::from(["api".into(), "oauth".into()]));
}

#[test]
fn unknown_and_incomplete_refresh_material_fail_closed_without_secret_rewriting() {
    for (token, endpoint) in [
        (None, None),
        (Some("refresh"), None),
        (None, Some("https://example.invalid/token")),
        (Some(" \t"), Some("https://example.invalid/token")),
        (Some("refresh"), Some(" \t")),
    ] {
        let mut doc = document(Some("oauth_only"));
        doc.providers[0].protocol_profile = Some("chatgpt_codex_oauth_official_api".into());
        let credential = &mut doc.providers[0].credentials[1];
        credential.session_auth =
            Some(serde_json::from_value(json!({"transport":"bearer"})).unwrap());
        credential.refresh_token = token.map(str::to_owned);
        credential.refresh_endpoint = endpoint.map(str::to_owned);
        // Existing URL validation rejects blank endpoints before pool compilation.
        if endpoint.is_some_and(|endpoint| endpoint.trim().is_empty()) {
            assert!(RouteConfigStore::from_document(doc).is_err());
            continue;
        }
        let store = RouteConfigStore::from_document(doc).unwrap();
        let route = store.snapshot();
        assert!(route.resolve_candidates(Some("model")).is_empty());
        assert!(route.list_models().is_empty());
        assert_eq!(
            route.document().providers[0].credentials[1]
                .refresh_token
                .as_deref(),
            token
        );
        assert_eq!(
            route.document().providers[0].credentials[1]
                .refresh_endpoint
                .as_deref(),
            endpoint
        );
    }
}

#[test]
fn disabled_or_unauthorized_oauth_does_not_select_an_api_key_or_outside_account() {
    let mut doc = document(Some("oauth_only"));
    doc.account_groups = vec![
        serde_json::from_value(json!({"id":"api-only", "name":"API only",
        "provider_credential_ids":["api"]}))
        .unwrap(),
    ];
    let store = RouteConfigStore::from_document(doc.clone()).unwrap();
    assert!(store
        .snapshot()
        .resolve_candidates_for_account_group(Some("model"), Some("api-only"))
        .unwrap()
        .is_empty());
    doc.providers[0].credentials[1].enabled = Some(false);
    let store = RouteConfigStore::from_document(doc).unwrap();
    assert!(store
        .snapshot()
        .resolve_candidates(Some("model"))
        .is_empty());
}

#[test]
fn base_payload_is_unknown_and_strict_policy_is_not_a_cross_provider_gate() {
    let mut doc = document(Some("oauth_only"));
    doc.providers[0].credentials.clear();
    doc.providers[0].api_key = "synthetic-base".into();
    doc.providers[0]
        .model_map_targets
        .insert("model".into(), vec!["upstream".into()]);
    assert!(RouteConfigStore::from_document(doc.clone())
        .unwrap()
        .snapshot()
        .resolve_candidates(Some("model"))
        .is_empty());
    let strict = RouteConfigStore::from_document(doc.clone()).unwrap();
    assert!(strict.snapshot().list_models().is_empty());
    assert_eq!(
        model_for_probe(
            &strict.snapshot().compiled.providers[0],
            &provider_default_account_id("pool"),
            "model"
        )
        .as_deref(),
        Some("upstream")
    );
    let mut other = document(None).providers.remove(0);
    other.id = "other".into();
    other.credentials.clear();
    other.api_key = "synthetic-other".into();
    doc.providers.push(other);
    let candidates = RouteConfigStore::from_document(doc)
        .unwrap()
        .snapshot()
        .resolve_candidates(Some("model"));
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].provider_account_id, "other");
}

#[test]
fn mapped_and_single_alias_paths_reject_pending_or_unsupported_oauth() {
    let mut doc = document(Some("oauth_only"));
    doc.providers[0]
        .model_map
        .insert("model".into(), "upstream".into());
    doc.providers[0].credentials[0].supported_models = vec!["upstream".into()];
    doc.providers[0].credentials[1].supported_models = vec!["unrelated".into()];
    let store = RouteConfigStore::from_document(doc).unwrap();
    assert!(store
        .snapshot()
        .resolve_candidates(Some("model"))
        .is_empty());
    assert!(store.snapshot().list_models().is_empty());
    let mut provider = store.snapshot().compiled.providers[0].clone();
    provider.credential_pool[1].supported_models.clear();
    provider.credential_pool[1].discovery_required = true;
    provider
        .model_map_targets
        .insert("model".into(), vec!["upstream".into()]);
    assert!(
        candidates::select_credential(&provider, Some("model"), Some("upstream"), None).is_none()
    );
    assert!(model_mapping::select_mapped_payload(&provider, "model", None).is_none());
}

#[test]
fn policy_changes_participate_in_fingerprint_reuse_and_retired_selection() {
    let mut doc = document(Some("oauth_only"));
    let store = RouteConfigStore::from_document(doc.clone()).unwrap();
    let old = store.snapshot();
    let owner = old.compiled.providers[0].credential_pool[1]
        .refresh_config
        .as_ref()
        .unwrap();
    doc.aliases.insert("alias".into(), "model".into());
    let reused = store.replace_document(doc.clone()).unwrap();
    let current = reused.compiled.providers[0].credential_pool[1]
        .refresh_config
        .as_ref()
        .unwrap();
    assert!(Arc::ptr_eq(owner, current));
    assert!(owner.is_active());
    doc.providers[0].credential_pool_auth_policy = CredentialPoolAuthPolicy::Any;
    let changed = store.replace_document(doc).unwrap();
    assert!(!owner.is_active());
    assert!(!Arc::ptr_eq(
        owner,
        changed.compiled.providers[0].credential_pool[1]
            .refresh_config
            .as_ref()
            .unwrap()
    ));
    assert!(old.resolve_candidates(Some("model")).is_empty());
    assert_eq!(
        changed.resolve_candidates(Some("model"))[0]
            .payload
            .credential_id
            .as_deref(),
        Some("api")
    );
}
