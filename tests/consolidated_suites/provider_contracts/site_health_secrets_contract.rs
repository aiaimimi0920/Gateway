//! Monitoring secrets reuse the route-secret API and stable provider/plan identity.
use neuro_gateway::{
    console::secrets::{redact_route_document, resolve_secret_patches, SecretPatch},
    routing::config::RouteConfigYaml,
};
use serde_json::json;

fn document() -> RouteConfigYaml {
    serde_json::from_value(json!({"providers":[
        {"id":"other","adapter":"openai_compatible","base_url":"https://other.invalid/v1"},
        {"id":"site","adapter":"openai_compatible","base_url":"https://site.invalid/v1","site_health":{
            "adapter":"sub2api-v2","auth":{"kind":"bearer","token":"legacy-secret"},
            "profiles":[
                {"id":"plus","name":"Plus","account_group_ids":["plus"],"policy":{"adapter":"sub2api-v2","auth":{"kind":"bearer","token":"plus-secret"}}},
                {"id":"pro","name":"Pro","account_group_ids":["pro"],"policy":{"adapter":"sub2api-v2","auth":{"kind":"bearer","token":"pro-secret"}}}
            ]}}
    ]})).unwrap()
}

#[test]
fn redaction_and_keep_follow_provider_and_plan_reordering_in_both_directions() {
    let active = document();
    let redacted = redact_route_document(&active).unwrap();
    let text = serde_json::to_string(&redacted.document).unwrap();
    assert!(!text.contains("-secret"));
    let monitoring: Vec<_> = redacted
        .secrets
        .iter()
        .filter(|s| s.path.contains("/site_health/"))
        .collect();
    assert_eq!(monitoring.len(), 3);
    assert!(monitoring.iter().all(|s| s.configured));
    let mut draft = redacted.document;
    draft.providers.swap(0, 1);
    let mut value = serde_json::to_value(&draft).unwrap();
    value["providers"][0]["site_health"]["profiles"]
        .as_array_mut()
        .unwrap()
        .swap(0, 1);
    let draft = serde_json::from_value(value).unwrap();
    let patches = monitoring
        .iter()
        .map(|s| SecretPatch::keep(s.path.replace("/providers/1/", "/providers/0/")))
        .collect::<Vec<_>>();
    let result =
        serde_json::to_value(resolve_secret_patches(&active, draft, &patches).unwrap()).unwrap();
    assert_eq!(
        result["providers"][0]["site_health"]["profiles"][0]["policy"]["auth"]["token"],
        "pro-secret"
    );
    assert_eq!(
        result["providers"][0]["site_health"]["profiles"][1]["policy"]["auth"]["token"],
        "plus-secret"
    );
    assert_eq!(
        result["providers"][0]["site_health"]["auth"]["token"],
        "legacy-secret"
    );
}

#[test]
fn embedded_tokens_and_same_slot_identity_reuse_are_rejected() {
    let active = document();
    let mut draft = redact_route_document(&active).unwrap().document;
    let mut value = serde_json::to_value(&draft).unwrap();
    value["providers"][1]["site_health"]["profiles"][0]["id"] = json!("replacement");
    draft = serde_json::from_value(value).unwrap();
    let path = "/providers/1/site_health/profiles/0/policy/auth/token";
    assert!(resolve_secret_patches(&active, draft.clone(), &[SecretPatch::keep(path)]).is_err());
    assert!(resolve_secret_patches(&active, active.clone(), &[]).is_err());
    let redacted = redact_route_document(&active).unwrap();
    let mut patches = redacted
        .secrets
        .iter()
        .filter(|s| s.configured)
        .map(|s| SecretPatch::keep(&s.path))
        .collect::<Vec<_>>();
    patches
        .iter_mut()
        .find(|p| p.path == path)
        .unwrap()
        .operation = neuro_gateway::console::secrets::SecretOperation::Replace;
    patches.iter_mut().find(|p| p.path == path).unwrap().value = Some(json!("new-secret"));
    assert!(resolve_secret_patches(&active, draft, &patches).is_ok());
}

#[test]
fn deleted_plans_drop_their_secret_and_missing_or_cross_namespace_sources_cannot_keep() {
    let active = document();
    let redacted = redact_route_document(&active).unwrap();
    let mut value = serde_json::to_value(&redacted.document).unwrap();
    value["providers"][1]["site_health"]["profiles"]
        .as_array_mut()
        .unwrap()
        .remove(1);
    let patches = redacted
        .secrets
        .iter()
        .filter(|s| s.configured && !s.path.contains("/profiles/1/"))
        .map(|s| SecretPatch::keep(&s.path))
        .collect::<Vec<_>>();
    assert!(
        resolve_secret_patches(&active, serde_json::from_value(value).unwrap(), &patches).is_ok()
    );
    let mut value = serde_json::to_value(&redacted.document).unwrap();
    value["providers"][1]["site_health"]["profiles"][1]["id"] = json!("plus");
    assert!(redact_route_document(&serde_json::from_value(value).unwrap()).is_err());
    let mut source = serde_json::to_value(&active).unwrap();
    source["providers"][1]["site_health"]["profiles"] = json!([]);
    let source = serde_json::from_value(source).unwrap();
    assert!(resolve_secret_patches(
        &source,
        redacted.document,
        &[SecretPatch::keep(
            "/providers/1/site_health/profiles/0/policy/auth/token"
        )]
    )
    .is_err());
}

#[test]
fn keep_cannot_send_an_existing_monitor_token_to_a_changed_audience() {
    let active = document();
    let redacted = redact_route_document(&active).unwrap();
    let patches: Vec<_> = redacted
        .secrets
        .iter()
        .filter(|s| s.configured)
        .map(|s| SecretPatch::keep(&s.path))
        .collect();
    for (path, value) in [
        (
            "/providers/1/base_url",
            json!("https://attacker.invalid/v1"),
        ),
        (
            "/providers/1/site_health/site_url",
            json!("https://attacker.invalid"),
        ),
        (
            "/providers/1/site_health/profiles/0/policy/site_url",
            json!("https://attacker.invalid"),
        ),
        (
            "/providers/1/site_health/profiles/0/policy/adapter",
            json!("model-status-all"),
        ),
        (
            "/providers/1/site_health/profiles/0/policy/auth/kind",
            json!("api_key"),
        ),
        (
            "/providers/1/site_health/profiles/0/policy/allow_private_network",
            json!(true),
        ),
    ] {
        let mut draft = serde_json::to_value(&redacted.document).unwrap();
        *draft.pointer_mut(path).unwrap() = value;
        let error =
            resolve_secret_patches(&active, serde_json::from_value(draft).unwrap(), &patches)
                .unwrap_err();
        assert_eq!(error.code(), "secret_keep_destination_changed", "{path}");
    }
    let mut draft = redacted.document;
    draft.providers[1].base_url = "https://site.invalid/another/api/path".into();
    assert!(
        resolve_secret_patches(&active, draft, &patches).is_ok(),
        "fixed recipes depend on origin, not business API path"
    );
    let mut draft = serde_json::to_value(redact_route_document(&active).unwrap().document).unwrap();
    draft["providers"][1]["site_health"]["profiles"][0]["policy"]["site_url"] =
        json!("https://new.invalid");
    let mut authorized = patches;
    let path = "/providers/1/site_health/profiles/0/policy/auth/token";
    *authorized.iter_mut().find(|p| p.path == path).unwrap() =
        SecretPatch::replace(path, json!("new-monitor-secret"));
    assert!(
        resolve_secret_patches(&active, serde_json::from_value(draft).unwrap(), &authorized)
            .is_ok()
    );
}

#[test]
fn family_negotiation_requires_explicit_secret_authorization_instead_of_silent_migration() {
    for (old, new) in [
        ("new-api-perf-summary", "new-api"),
        ("sub2api-v2", "sub2api"),
        ("model-health-slot", "compatible-gateway"),
        ("new-api", "one-api"),
        ("new-api", "compatible-gateway"),
    ] {
        let mut active = serde_json::to_value(document()).unwrap();
        active["providers"][1]["site_health"]["profiles"][0]["policy"]["adapter"] = json!(old);
        let active: RouteConfigYaml = serde_json::from_value(active).unwrap();
        let redacted = redact_route_document(&active).unwrap();
        let mut patches: Vec<_> = redacted
            .secrets
            .iter()
            .filter(|s| s.configured)
            .map(|s| SecretPatch::keep(&s.path))
            .collect();
        let mut draft = serde_json::to_value(redacted.document).unwrap();
        draft["providers"][1]["site_health"]["profiles"][0]["policy"]["adapter"] = json!(new);
        let draft: RouteConfigYaml = serde_json::from_value(draft).unwrap();
        assert_eq!(
            resolve_secret_patches(&active, draft.clone(), &patches)
                .unwrap_err()
                .code(),
            "secret_keep_destination_changed",
            "{old} -> {new}"
        );
        let path = "/providers/1/site_health/profiles/0/policy/auth/token";
        *patches.iter_mut().find(|p| p.path == path).unwrap() =
            SecretPatch::replace(path, json!("authorized-monitor-token"));
        assert!(resolve_secret_patches(&active, draft, &patches).is_ok());
    }
}
