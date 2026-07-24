use std::sync::Arc;

use neuro_gateway::console::document::validate_route_document;
use neuro_gateway::console::revision::{RevisionActor, RevisionMetadata};
use neuro_gateway::routing::config::{ActiveConfigSource, RouteConfigStore, RouteConfigYaml};
use time::OffsetDateTime;
use tokio::sync::Barrier;

fn document(provider_id: &str, model: &str, alias_target: &str) -> RouteConfigYaml {
    serde_yaml::from_str(&format!(
        r#"
providers:
  - id: {provider_id}
    base_url: https://example.com/v1
    api_key: test-secret
    supported_models: [{model}]
model_routes:
  - pattern: {model}
    provider_ids: [{provider_id}]
aliases:
  answer: {alias_target}
"#
    ))
    .expect("fixture YAML must parse")
}

fn revision_for(store: &RouteConfigStore, document: &RouteConfigYaml) -> RevisionMetadata {
    let validated = validate_route_document(document.clone()).expect("fixture must validate");
    RevisionMetadata::from_validated(
        store.snapshot().revision().sequence() + 1,
        Some(store.snapshot().revision().id().to_string()),
        RevisionActor::ManagementToken,
        OffsetDateTime::now_utc(),
        Some("test replacement".to_string()),
        &validated,
    )
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_readers_only_observe_complete_revisions() {
    const READER_COUNT: usize = 4;

    let old = document("old-provider", "old-model", "old-model");
    let new = document("new-provider", "new-model", "new-model");
    let store = Arc::new(RouteConfigStore::from_document(old).expect("old fixture"));
    let old_revision = store.snapshot().revision().id().to_string();
    let phase_barrier = Arc::new(Barrier::new(READER_COUNT + 1));
    let readers = (0..READER_COUNT)
        .map(|_| {
            let store = Arc::clone(&store);
            let phase_barrier = Arc::clone(&phase_barrier);
            tokio::spawn(async move {
                let observe = || {
                    let snapshot = store.snapshot();
                    let revision = snapshot.revision().id().to_string();
                    let alias = snapshot.resolve_alias(Some("answer"));
                    let candidate = snapshot
                        .resolve_candidates(alias.as_deref())
                        .first()
                        .map(|candidate| candidate.provider_account_id.clone());
                    let model = snapshot
                        .list_models()
                        .into_iter()
                        .find(|model| model.id == "old-model" || model.id == "new-model")
                        .map(|model| model.id);
                    (revision, alias, candidate, model)
                };

                let old_observation = observe();
                phase_barrier.wait().await;
                phase_barrier.wait().await;
                let new_observation = observe();
                [old_observation, new_observation]
            })
        })
        .collect::<Vec<_>>();

    phase_barrier.wait().await;
    let revision = revision_for(&store, &new);
    let validated = validate_route_document(new).expect("new fixture");
    store
        .replace_validated(validated, revision)
        .expect("replacement must succeed");
    let new_revision = store.snapshot().revision().id().to_string();
    phase_barrier.wait().await;

    let mut observations = Vec::new();
    for reader in readers {
        observations.extend(reader.await.expect("reader task"));
    }

    assert!(observations
        .iter()
        .any(|(revision, _, _, _)| revision == &old_revision));
    assert!(observations
        .iter()
        .any(|(revision, _, _, _)| revision == &new_revision));
    assert!(observations
        .iter()
        .all(|(revision, alias, provider, model)| {
            matches!(
                (
                    revision == &old_revision,
                    revision == &new_revision,
                    alias.as_deref(),
                    provider.as_deref(),
                    model.as_deref()
                ),
                (
                    true,
                    false,
                    Some("old-model"),
                    Some("old-provider"),
                    Some("old-model")
                ) | (
                    false,
                    true,
                    Some("new-model"),
                    Some("new-provider"),
                    Some("new-model")
                )
            )
        }));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn concurrent_writers_with_the_same_lineage_allow_exactly_one_replacement() {
    let old = document("old-provider", "old-model", "old-model");
    let store = Arc::new(RouteConfigStore::from_document(old).expect("old fixture"));
    let parent = store.snapshot().revision().clone();
    let sequence = parent.sequence() + 1;

    let first = validate_route_document(document("first-provider", "first-model", "first-model"))
        .expect("first fixture");
    let first_revision = RevisionMetadata::from_validated(
        sequence,
        Some(parent.id().to_string()),
        RevisionActor::ManagementToken,
        OffsetDateTime::now_utc(),
        Some("first writer".to_string()),
        &first,
    );
    let second =
        validate_route_document(document("second-provider", "second-model", "second-model"))
            .expect("second fixture");
    let second_revision = RevisionMetadata::from_validated(
        sequence,
        Some(parent.id().to_string()),
        RevisionActor::ManagementToken,
        OffsetDateTime::now_utc(),
        Some("second writer".to_string()),
        &second,
    );

    let start = Arc::new(Barrier::new(3));
    let first_writer = {
        let store = Arc::clone(&store);
        let start = Arc::clone(&start);
        tokio::spawn(async move {
            start.wait().await;
            store.replace_validated(first, first_revision)
        })
    };
    let second_writer = {
        let store = Arc::clone(&store);
        let start = Arc::clone(&start);
        tokio::spawn(async move {
            start.wait().await;
            store.replace_validated(second, second_revision)
        })
    };

    start.wait().await;
    let outcomes = [
        first_writer.await.expect("first writer task"),
        second_writer.await.expect("second writer task"),
    ];
    let mut successes = 0;
    let mut conflicts = 0;
    for outcome in outcomes {
        match outcome {
            Ok(_) => successes += 1,
            Err(error) => {
                assert_eq!(error.code(), "console_revision_conflict");
                conflicts += 1;
            }
        }
    }

    assert_eq!(successes, 1);
    assert_eq!(conflicts, 1);
}

#[test]
fn replacement_rejects_revision_digest_mismatch_without_swapping() {
    let old = document("old-provider", "old-model", "old-model");
    let new = document("new-provider", "new-model", "new-model");
    let store = RouteConfigStore::from_document(old).expect("old fixture");
    let validated = validate_route_document(new).expect("new fixture");
    let wrong_revision = RevisionMetadata::from_validated(
        1,
        Some(store.snapshot().revision().id().to_string()),
        RevisionActor::ManagementToken,
        OffsetDateTime::now_utc(),
        None,
        &validate_route_document(document(
            "different-provider",
            "different-model",
            "different-model",
        ))
        .expect("different fixture"),
    );

    let error = store
        .replace_validated(validated, wrong_revision)
        .expect_err("digest mismatch must be rejected");
    assert_eq!(error.code(), "console_revision_digest_mismatch");
    assert_eq!(store.snapshot().provider_count(), 1);
    assert_eq!(
        store.snapshot().resolve_alias(Some("answer")),
        Some("old-model".to_string())
    );
}

#[test]
fn unchanged_revision_is_rejected_as_a_noop() {
    let old = document("old-provider", "old-model", "old-model");
    let store = RouteConfigStore::from_document(old.clone()).expect("fixture");
    let validated = validate_route_document(old).expect("fixture");
    let current = store.snapshot().revision().clone();
    let revision = RevisionMetadata::from_validated(
        current.sequence() + 1,
        Some(current.id().to_string()),
        RevisionActor::ManagementToken,
        OffsetDateTime::now_utc(),
        None,
        &validated,
    );

    let error = store
        .replace_validated(validated, revision)
        .expect_err("same active document must be rejected");
    assert_eq!(error.code(), "console_revision_unchanged");
}

#[test]
fn same_document_with_stale_revision_is_a_lineage_conflict() {
    let old = document("old-provider", "old-model", "old-model");
    let store = RouteConfigStore::from_document(old.clone()).expect("fixture");
    let validated = validate_route_document(old).expect("fixture");
    let stale_revision = store.snapshot().revision().clone();

    let error = store
        .replace_validated(validated, stale_revision)
        .expect_err("stale metadata must not be classified as a no-op");
    assert_eq!(error.code(), "console_revision_conflict");
}

#[test]
fn same_document_with_wrong_parent_is_a_lineage_conflict() {
    let old = document("old-provider", "old-model", "old-model");
    let store = RouteConfigStore::from_document(old.clone()).expect("fixture");
    let validated = validate_route_document(old).expect("fixture");
    let wrong_parent = RevisionMetadata::from_validated(
        1,
        Some("r0-aaaaaaaaaaaa".to_string()),
        RevisionActor::ManagementToken,
        OffsetDateTime::now_utc(),
        None,
        &validated,
    );

    let error = store
        .replace_validated(validated, wrong_parent)
        .expect_err("wrong parent must not be classified as a no-op");
    assert_eq!(error.code(), "console_revision_conflict");
}

#[test]
fn replacement_revision_must_descend_from_the_active_revision() {
    let old = document("old-provider", "old-model", "old-model");
    let new = document("new-provider", "new-model", "new-model");
    let store = RouteConfigStore::from_document(old).expect("old fixture");
    let validated = validate_route_document(new).expect("new fixture");
    let unrelated_revision = RevisionMetadata::from_validated(
        2,
        Some("r1-aaaaaaaaaaaa".to_string()),
        RevisionActor::ManagementToken,
        OffsetDateTime::now_utc(),
        None,
        &validated,
    );

    let error = store
        .replace_validated(validated, unrelated_revision)
        .expect_err("unrelated parent must be rejected");
    assert_eq!(error.code(), "console_revision_conflict");
    assert_eq!(
        store.snapshot().resolve_alias(Some("answer")),
        Some("old-model".to_string())
    );
}

#[test]
fn recovered_install_accepts_a_monotonic_revision_gap() {
    let old = document("old-provider", "old-model", "old-model");
    let new = document("new-provider", "new-model", "new-model");
    let store = RouteConfigStore::from_document(old).expect("old fixture");
    let validated = validate_route_document(new).expect("new fixture");
    let recovered_revision = RevisionMetadata::from_validated(
        4,
        Some("r3-aaaaaaaaaaaa".to_string()),
        RevisionActor::Recovery,
        OffsetDateTime::now_utc(),
        Some("redis recovery".to_string()),
        &validated,
    );

    let snapshot = store
        .install_external_validated(validated, recovered_revision, ActiveConfigSource::Recovered)
        .expect("recovered snapshot must install");

    assert_eq!(snapshot.source(), ActiveConfigSource::Recovered);
    assert_eq!(snapshot.revision().sequence(), 4);
    assert_eq!(
        snapshot.resolve_alias(Some("answer")),
        Some("new-model".to_string())
    );
}

#[test]
fn recovered_install_allows_a_newer_revision_with_identical_document() {
    let old = document("stable-provider", "stable-model", "stable-model");
    let store = RouteConfigStore::from_document(old.clone()).expect("fixture");
    let validated = validate_route_document(old).expect("fixture");
    let revision = RevisionMetadata::from_validated(
        store.snapshot().revision().sequence() + 5,
        Some("r4-aaaaaaaaaaaa".to_string()),
        RevisionActor::Recovery,
        OffsetDateTime::now_utc(),
        Some("same document newer revision".to_string()),
        &validated,
    );

    let snapshot = store
        .install_external_validated(validated, revision, ActiveConfigSource::Recovered)
        .expect("same document recovered revision must install");

    assert_eq!(snapshot.revision().sequence(), 5);
    assert_eq!(snapshot.source(), ActiveConfigSource::Recovered);
}

#[test]
fn legacy_yaml_startup_keeps_dangling_routes_with_repair_diagnostics() {
    let path = std::env::temp_dir().join(format!(
        "gateway-console-hot-reload-{}-routes.yaml",
        std::process::id()
    ));
    std::fs::write(
        &path,
        r#"
providers:
  - id: configured-provider
    base_url: https://example.com/v1
    api_key: test-secret
model_routes:
  - pattern: legacy-model
    provider_ids: [historical-provider]
aliases: {}
"#,
    )
    .expect("write fixture");

    let store = RouteConfigStore::load_from_yaml(&path).expect("legacy startup remains tolerant");
    let snapshot = store.snapshot();
    assert_eq!(snapshot.source(), ActiveConfigSource::Yaml);
    assert!(snapshot.diagnostics().requires_repair());
    assert_eq!(snapshot.revision().sequence(), 0);
    assert!(snapshot.has_routes());

    let _ = std::fs::remove_file(path);
}

#[test]
fn identical_provider_reuses_round_robin_runtime_state() {
    let old = document_with_credentials("stable-provider", &["key-a", "key-b"]);
    let mut new = document_with_credentials("stable-provider", &["key-a", "key-b"]);
    new.aliases
        .insert("stable-alias".to_string(), "stable-model".to_string());
    let store = RouteConfigStore::from_document(old).expect("old fixture");

    let first = store
        .resolve_candidates(Some("stable-model"))
        .first()
        .expect("candidate")
        .payload
        .api_key
        .clone();
    let validated = validate_route_document(new).expect("new fixture");
    let revision = revision_for(&store, validated.document());
    store
        .replace_validated(validated, revision)
        .expect("replacement");
    let second = store
        .resolve_candidates(Some("stable-model"))
        .first()
        .expect("candidate")
        .payload
        .api_key
        .clone();

    assert_eq!(first, "key-a");
    assert_eq!(second, "key-b");
}

#[test]
fn changed_provider_does_not_reuse_round_robin_runtime_state() {
    let old = document_with_credentials("changed-provider", &["key-a", "key-b"]);
    let mut new = document_with_credentials("changed-provider", &["key-a", "key-c"]);
    new.aliases
        .insert("changed-alias".to_string(), "stable-model".to_string());
    let store = RouteConfigStore::from_document(old).expect("old fixture");

    assert_eq!(
        store.resolve_candidates(Some("stable-model"))[0]
            .payload
            .api_key,
        "key-a"
    );
    let validated = validate_route_document(new).expect("new fixture");
    let revision = revision_for(&store, validated.document());
    store
        .replace_validated(validated, revision)
        .expect("replacement");

    let first_after_replace = store.resolve_candidates(Some("stable-model"))[0]
        .payload
        .api_key
        .clone();
    let second_after_replace = store.resolve_candidates(Some("stable-model"))[0]
        .payload
        .api_key
        .clone();
    assert_eq!(first_after_replace, "key-a");
    assert_eq!(second_after_replace, "key-c");
}

#[test]
fn legacy_anonymous_credentials_reuse_state_after_route_only_replacement() {
    let yaml = r#"
providers:
  - id: legacy-provider
    base_url: https://example.com/v1
    credentials:
      - api_key: key-a
      - api_key: key-b
model_routes:
  - pattern: legacy-model
    provider_ids: [legacy-provider]
aliases: {}
"#;
    let path = std::env::temp_dir().join(format!(
        "gateway-console-anonymous-{}-routes.yaml",
        std::process::id()
    ));
    std::fs::write(&path, yaml).expect("write fixture");
    let store = RouteConfigStore::load_from_yaml(&path).expect("legacy fixture");
    let _ = std::fs::remove_file(path);

    assert_eq!(
        store.resolve_candidates(Some("legacy-model"))[0]
            .payload
            .api_key,
        "key-a"
    );
    let mut replacement: RouteConfigYaml = serde_yaml::from_str(yaml).expect("parse fixture");
    replacement
        .aliases
        .insert("legacy-alias".to_string(), "legacy-model".to_string());
    let validated = validate_route_document(replacement).expect("strict replacement");
    let revision = revision_for(&store, validated.document());
    store
        .replace_validated(validated, revision)
        .expect("replacement");

    assert_eq!(
        store.resolve_candidates(Some("legacy-model"))[0]
            .payload
            .api_key,
        "key-b"
    );
}

fn document_with_credentials(provider_id: &str, keys: &[&str]) -> RouteConfigYaml {
    let credentials = keys
        .iter()
        .enumerate()
        .map(|(index, key)| format!("      - id: cred-{index}\n        api_key: {key}"))
        .collect::<Vec<_>>()
        .join("\n");
    serde_yaml::from_str(&format!(
        r#"
providers:
  - id: {provider_id}
    base_url: https://example.com/v1
    credentials:
{credentials}
model_routes:
  - pattern: stable-model
    provider_ids: [{provider_id}]
aliases: {{}}
"#
    ))
    .expect("credential fixture must parse")
}
