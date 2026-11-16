//! Grok/Qwen material must survive the real local authority, not a mock Redis cache.
use neuro_gateway::console::{
    ConsoleConfig, ConsoleConfigValues, RouteConfigRuntime, TransactionPhase,
};
use neuro_gateway::routing::config::{RouteConfigSnapshot, RouteConfigStore, RouteConfigYaml};
use serde_json::{json, Value};
use std::fs;
#[path = "console_transaction_support/mod.rs"]
mod support;
use support::{stage_transaction, FakeRedisMode, RuntimeHarness};
#[path = "console_web_session_restart_contract/management.rs"]
mod management;

fn document(grok: bool, rotated: bool) -> RouteConfigYaml {
    let (preset, base, adapter, model, cookie) = if grok {
        (
            "grok",
            "https://grok.com",
            "grok_web",
            "grok-chat-fast",
            "sso",
        )
    } else {
        (
            "qwen-web-chat",
            "https://chat.qwen.ai/api/v1",
            "qwen_web",
            "qwen3.7-plus",
            "token",
        )
    };
    let token = if rotated {
        "synthetic-new"
    } else {
        "synthetic-old"
    };
    let mut selected = json!({"id":"exact","api_key":token,
        "headers":{"Cookie":format!("{cookie}={token}; cf_clearance=synthetic-clearance"),
        "User-Agent":"synthetic-UA"},"supported_models":["public-alias"]});
    // Old documents omit session_auth; new imports carry the product-specific contract.
    if rotated {
        selected["session_auth"] = if grok {
            json!({"transport":"cookie","primaryCookieName":"sso","secondaryCookieName":"sso-rw"})
        } else {
            json!({"transport":"bearer"})
        };
        if grok {
            selected["headers"]["Cookie"] = json!(format!(
                "sso={token}; sso-rw={token}; cf_clearance=synthetic-clearance"
            ));
        }
    }
    serde_json::from_value(json!({"providers":[{"id":"selected","preset":preset,
        "base_url":base,"explicit_credentials_only":true,"supported_models":["public-alias","other-model"],
        "model_map":{"public-alias":model},"call_flow":{"special_adapter":adapter,"fingerprint_adapter":"session"},
        "credentials":[selected,{"id":"other","api_key":"synthetic-other","supported_models":["other-model"]}]}],
        "model_routes":[{"pattern":"public-alias","provider_ids":["selected"]}]})).unwrap()
}

fn config(harness: &RuntimeHarness) -> ConsoleConfig {
    ConsoleConfig::from_values(ConsoleConfigValues {
        state_dir: Some(
            harness
                .persistence
                .state_root()
                .parent()
                .unwrap()
                .to_path_buf(),
        ),
        routes_file: Some(harness.routes.clone()),
        ..Default::default()
    })
    .unwrap()
}

fn value(document: &RouteConfigYaml) -> Value {
    serde_json::to_value(document).unwrap()
}

fn assert_routes(snapshot: &RouteConfigSnapshot, expected: &RouteConfigYaml) {
    let expected = RouteConfigStore::from_document(expected.clone()).unwrap();
    let projection = |snapshot: &RouteConfigSnapshot, model| {
        snapshot
            .resolve_candidates(Some(model))
            .into_iter()
            .map(|candidate| {
                json!([
                    candidate.provider_account_id,
                    candidate.provider_credential_id,
                    candidate.upstream_model,
                    candidate.payload
                ])
            })
            .collect::<Vec<_>>()
    };
    for model in ["public-alias", "other-model"] {
        assert_eq!(
            projection(snapshot, model),
            projection(&expected.snapshot(), model)
        );
    }
}

async fn reopen(harness: &RuntimeHarness, expected: &RouteConfigYaml, revision: &str) {
    // Each iteration constructs fresh runtime/store owners, using only on-disk authority.
    for _ in 0..2 {
        let runtime = RouteConfigRuntime::new_local(&config(harness)).unwrap();
        runtime.recover_startup().await.unwrap();
        let snapshot = runtime.route_config().snapshot();
        assert_eq!(snapshot.revision().id(), revision);
        assert_routes(&snapshot, expected);
        let archived = harness
            .persistence
            .load_revision(revision)
            .unwrap()
            .unwrap();
        assert_eq!(value(archived.document()), value(expected));
        let yaml = RouteConfigStore::load_from_yaml(&harness.routes).unwrap();
        assert_routes(&yaml.snapshot(), expected);
        let yaml_document: RouteConfigYaml =
            serde_yaml::from_slice(&fs::read(&harness.routes).unwrap()).unwrap();
        assert_eq!(value(&yaml_document), value(expected));
    }
}

#[tokio::test]
async fn web_session_old_read_rotation_and_clear_survive_repeated_local_restart() {
    for grok in [true, false] {
        let original = document(grok, false);
        let harness = RuntimeHarness::new(
            "web-session-restart",
            original.clone(),
            FakeRedisMode::Activated,
            true,
        );
        let original_yaml = fs::read(&harness.routes).unwrap();
        let runtime = RouteConfigRuntime::new_local(&config(&harness)).unwrap();
        runtime.recover_startup().await.unwrap();
        let initial = runtime.route_config().snapshot();
        assert_routes(&initial, &original);
        assert_eq!(
            fs::read(&harness.routes).unwrap(),
            original_yaml,
            "legacy read must not rewrite YAML"
        );
        let old_revision = initial.revision().id().to_owned();
        drop(initial);
        drop(runtime);
        reopen(&harness, &original, &old_revision).await;

        let rotated = document(grok, true);
        let runtime = RouteConfigRuntime::new_local(&config(&harness)).unwrap();
        runtime.recover_startup().await.unwrap();
        let saved = runtime
            .commit_document(&old_revision, rotated.clone(), None)
            .await
            .unwrap();
        let saved_revision = saved.revision().id().to_owned();
        assert_ne!(saved_revision, old_revision);
        assert_eq!(
            value(&original)["providers"][0]["credentials"][1],
            value(
                harness
                    .persistence
                    .load_revision(saved.revision().id())
                    .unwrap()
                    .unwrap()
                    .document()
            )["providers"][0]["credentials"][1]
        );
        drop(saved);
        drop(runtime);
        reopen(&harness, &rotated, &saved_revision).await;

        let runtime = RouteConfigRuntime::new_local(&config(&harness)).unwrap();
        runtime.recover_startup().await.unwrap();
        let stale = runtime
            .commit_document(&old_revision, original.clone(), None)
            .await
            .unwrap_err();
        assert_eq!(stale.code(), "console_revision_conflict");
        // Explicit account removal must not resurrect its old material from an archive.
        let mut cleared = rotated.clone();
        cleared.providers[0].credentials.remove(0);
        let saved = runtime
            .commit_document(&saved_revision, cleared.clone(), None)
            .await
            .unwrap();
        let cleared_revision = saved.revision().id().to_owned();
        drop(saved);
        drop(runtime);
        reopen(&harness, &cleared, &cleared_revision).await;
        assert!(harness
            .journal
            .load_transactions()
            .unwrap()
            .iter()
            .all(|record| record.phase() == TransactionPhase::Committed));
    }
}

async fn interrupted_rotation(published: bool) {
    for grok in [true, false] {
        let original = document(grok, false);
        let rotated = document(grok, true);
        let harness = RuntimeHarness::new(
            "web-session-interrupted",
            original.clone(),
            FakeRedisMode::Activated,
            true,
        );
        let runtime = RouteConfigRuntime::new_local(&config(&harness)).unwrap();
        runtime.recover_startup().await.unwrap();
        let old_revision = runtime.route_config().snapshot().revision().id().to_owned();
        drop(runtime);
        // Existing transaction fixture stops after the real atomic YAML replacement.
        let staged = stage_transaction(&harness, &rotated, TransactionPhase::YamlReplaced);
        let new_revision = staged.redis_revision.metadata().id();
        if published {
            // Simulate a crash after local authority publication, before journal completion.
            fs::write(
                harness.persistence.state_root().join("local-active.json"),
                serde_json::to_vec(new_revision).unwrap(),
            )
            .unwrap();
        }
        let (expected, revision, phase) = if published {
            (&rotated, new_revision, TransactionPhase::Committed)
        } else {
            (&original, old_revision.as_str(), TransactionPhase::Aborted)
        };
        reopen(&harness, expected, revision).await;
        let records = harness.journal.load_transactions().unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].phase(), phase);
        // Preserve recovery archives, including the losing candidate; never delete secrets.
        assert!(harness
            .persistence
            .load_revision(new_revision)
            .unwrap()
            .is_some());
        assert!(harness
            .persistence
            .load_revision(&old_revision)
            .unwrap()
            .is_some());
    }
}

#[tokio::test]
async fn web_session_interrupted_before_publication_restores_old_material() {
    interrupted_rotation(false).await;
}

#[tokio::test]
async fn web_session_interrupted_after_publication_keeps_new_material() {
    interrupted_rotation(true).await;
}
