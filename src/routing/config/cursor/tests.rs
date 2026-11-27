use super::*;
use crate::routing::config::RouteConfigStore;
use serde_json::{json, Value};

fn document() -> Value {
    json!({
        "providers": [
            {"id":"ordinary", "base_url":"https://example.invalid", "supported_models":["ordinary"]}
        ],
        "model_routes": [{"pattern":"cursor-*","provider_ids":["ordinary"]}]
    })
}

fn decode(value: Value) -> RouteConfigYaml {
    serde_json::from_value(value).unwrap()
}

#[test]
fn cursor_legacy_acp_declarations_fail_closed_at_provider_and_credential_levels() {
    let mut provider_level = document();
    provider_level["providers"].as_array_mut().unwrap().insert(
        0,
        json!({
            "id":"my-cursor",
            "auth_mode": "cursor_acp",
            "base_url":"",
            "explicit_credentials_only":true,
            "credentials":[{"id":"my-cursor-account","enabled":false}]
        }),
    );
    assert!(RouteConfigStore::from_document(decode(provider_level)).is_err());

    let mut credential_level = document();
    credential_level["providers"][0]["credentials"] =
        json!([{"id":"unrelated","auth_mode":"cursor_acp"}]);
    assert!(RouteConfigStore::from_document(decode(credential_level)).is_err());
}

#[test]
fn cursor_private_model_stays_reserved_without_account_or_after_reload() {
    let store = RouteConfigStore::from_document(decode(document())).unwrap();
    let old = store.snapshot();
    let mut updated = document();
    updated["providers"][0]["label"] = json!("Reloaded ordinary pool");
    let reloaded = store.replace_document(decode(updated)).unwrap();
    assert_ne!(old.revision().id(), reloaded.revision().id());
    for snapshot in [old, reloaded] {
        assert!(!snapshot.list_models().iter().any(|m| is_model(&m.id)));
        for model in [MODEL, " cursor-personal ", "CURSOR-PERSONAL"] {
            let error = reject_unavailable_model(Some(model)).unwrap_err();
            assert!(!error.retryable);
            assert!(matches!(error.fallback_hint, FallbackHint::Abort { .. }));
        }
        assert!(reject_unavailable_model(Some("ordinary")).is_ok());
        for model in [None, Some("ordinary"), Some("cursor-other")] {
            assert!(!private_request(model));
        }
    }
}

fn metadata_provider(source: &str, model: &str) -> Value {
    let mut provider = json!({"id":"ordinary", "preset":"poe-openai",
        "base_url":"https://api.poe.com/v1", "explicit_credentials_only":true,
        "credentials":[{"id":"ordinary-account", "api_key":"synthetic-key"}]});
    match source {
        "discovery" => {
            let url = "https://example.invalid/v1";
            provider = json!({"id":"ordinary", "adapter":"openai_compatible",
                "protocol_profile":"openai_responses", "base_url":url,
                "credentials":[{"id":"ordinary-account", "api_key":"synthetic-key",
                    "discovery":{"source_url":url, "api_base":url,
                        "protocol":"chat_completions", "models":["ordinary",model],
                        "verified_models":["ordinary"], "checked_at":"2026-10-20T00:00:00Z",
                        "binding":crate::provider_discovery::binding(url,"synthetic-key")}}]});
        }
        "catalog" => {
            provider["credentials"][0]["model_catalog"] = json!({
                "sourceUrl":"https://api.poe.com/v1/models",
                "checkedAt":"2026-10-26T04:00:00Z", "models":[model]});
        }
        "manual" | "fetched" => {
            provider["credentials"][0]["model_selection"] = json!({"manual":[],"fetched":[]});
            provider["credentials"][0]["model_selection"][source] = json!([model]);
        }
        _ => unreachable!(),
    }
    provider
}

#[test]
fn cursor_reserved_metadata_cannot_feed_catalogue_or_scheduled_probes() {
    for source in ["discovery", "catalog", "manual", "fetched"] {
        let mut base = document();
        base["providers"][0] = metadata_provider(source, "cursor-other");
        let store = RouteConfigStore::from_document(decode(base.clone()))
            .unwrap_or_else(|error| panic!("{source} positive control: {error}"));
        base["providers"][0]["label"] = json!("Metadata reload positive control");
        let loaded = store.replace_document(decode(base.clone())).unwrap();
        assert!(loaded.get_providers()[0].credential_pool[0]
            .supported_models
            .iter()
            .any(|model| model == "cursor-other"));
        for model in [MODEL, "CURSOR-PERSONAL"] {
            let mut invalid = base.clone();
            invalid["providers"][0] = metadata_provider(source, model);
            let startup = RouteConfigStore::from_document(decode(invalid.clone()));
            let error = match startup {
                Ok(_) => panic!("{source} admitted reserved model {model}"),
                Err(error) => error,
            };
            assert!(error.to_string().contains("private Cursor ACP model"));
            assert!(store.replace_document(decode(invalid)).is_err());
            assert_eq!(store.snapshot().revision().id(), loaded.revision().id());
        }
    }
}

#[test]
fn cursor_reserved_aliases_and_catalogue_rejected_without_account() {
    let mut base = document();
    let store = RouteConfigStore::from_document(decode(base.clone())).unwrap();
    let revision = store.snapshot().revision().id().to_owned();
    let mut invalid_documents = Vec::new();
    for (field, value) in [
        ("aliases", json!({"shortcut":MODEL})),
        ("aliases", json!({(MODEL):"ordinary"})),
        ("aliases", json!({"shortcut":" CURSOR-PERSONAL "})),
        (
            "model_routes",
            json!([{"pattern":MODEL,"provider_ids":["ordinary"]}]),
        ),
    ] {
        let mut invalid = base.clone();
        invalid[field] = value;
        invalid_documents.push(invalid);
    }
    for (field, value) in [
        ("supported_models", json!([MODEL])),
        ("default_model", json!(MODEL)),
        ("model_map", json!({(MODEL):"ordinary"})),
        ("model_map_targets", json!({(MODEL):["ordinary"]})),
        (
            "credentials",
            json!([{"id":"ordinary-account","supported_models":[MODEL]}]),
        ),
    ] {
        let mut invalid = base.clone();
        invalid["providers"][0][field] = value;
        invalid_documents.push(invalid);
    }
    for invalid in invalid_documents {
        assert!(RouteConfigStore::from_document(decode(invalid.clone())).is_err());
        assert!(store.replace_document(decode(invalid)).is_err());
        assert_eq!(store.snapshot().revision().id(), revision);
    }
    // Reserve one exact identity, not all models with a Cursor-like prefix.
    base["aliases"] = json!({"shortcut":"cursor-other"});
    base["providers"][0]["supported_models"] = json!(["cursor-other"]);
    assert!(store.replace_document(decode(base)).is_ok());
}
