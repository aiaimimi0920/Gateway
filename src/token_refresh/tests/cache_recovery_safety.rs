//! Provenance is transaction-specific: identical material, orphan admission and same-R retries differ.
use super::{
    authority_fixture::Authority, cache_recovery::refresh, cache_wire::Cache, grok_wire::Mock, *,
};

fn directory(authority: &Authority) -> PathBuf {
    authority
        .persistence
        .state_root()
        .join("refresh-publications")
        .join(authority.active().metadata().id())
}

#[tokio::test]
async fn aborted_same_revision_candidate_does_not_block_or_impersonate_successful_retry() {
    let authority = Authority::new();
    let mock = Mock::new(200, grok_wire::rotated(), 0).await;
    let cache = Cache::new(false).await;
    authority.backend.bind_cache(&cache.pool);
    authority
        .backend
        .stall(authority_fixture::Stall::BeforeActivation);
    let mut pending = Pending::new();
    SERVER_AUTHORITY_BUDGET
        .scope(
            Duration::from_millis(80),
            refresh(&authority.runtime, &mock, &cache, &mut pending),
        )
        .await;
    assert_eq!(pending.len(), 1);
    authority.backend.resume();
    refresh(&authority.runtime, &mock, &cache, &mut pending).await;
    assert_eq!(pending.len(), 1);
    assert_eq!(std::fs::read_dir(directory(&authority)).unwrap().count(), 2);
    let records = crate::console::journal::TransactionJournal::new(authority.persistence.clone())
        .load_transactions()
        .unwrap();
    assert_eq!(records.len(), 2);
    assert!(records
        .iter()
        .any(|r| r.phase() == crate::console::TransactionPhase::Aborted));
    assert!(records
        .iter()
        .any(|r| r.phase() == crate::console::TransactionPhase::Committed));
    assert_eq!(records[0].new_revision(), records[1].new_revision());
    drop(pending);
    let restarted = authority.restart().await;
    cache.allow_writes();
    refresh(&restarted, &mock, &cache, &mut Pending::new()).await;
    assert_eq!(mock.count(), 1);
    assert_eq!(cache.commits(), 1);
    mock.stop().await;
    cache.stop().await;
}

#[tokio::test]
async fn orphan_candidate_is_not_executed_or_credited_as_a_publication() {
    let authority = Authority::new();
    let mock = Mock::new(200, grok_wire::rotated(), 0).await;
    let cache = Cache::new(false).await;
    authority.backend.bind_cache(&cache.pool);
    let mut pending = Pending::new();
    refresh(&authority.runtime, &mock, &cache, &mut pending).await;
    let path = std::fs::read_dir(directory(&authority))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let mut orphan: serde_json::Value =
        serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let tx = format!("tx-{}", uuid::Uuid::new_v4());
    orphan["intent"]["prepared"]["txId"] = serde_json::json!(tx);
    std::fs::write(
        directory(&authority).join(format!("{tx}.json")),
        serde_json::to_vec(&orphan).unwrap(),
    )
    .unwrap();
    drop(pending);
    let restarted = authority.restart().await;
    let settlements = authority.backend.settlements();
    cache.allow_writes();
    refresh(&restarted, &mock, &cache, &mut Pending::new()).await;
    assert_eq!(cache.commits(), 1);
    assert_eq!(
        authority.backend.settlements(),
        settlements + 1,
        "only the committed candidate is settled"
    );
    assert!(!authority
        .persistence
        .transaction_path(&tx)
        .unwrap()
        .exists());
    assert_eq!(mock.count(), 1);
    mock.stop().await;
    cache.stop().await;
}

#[tokio::test]
async fn deleted_and_readded_result_material_is_not_a_continuation_of_old_publication() {
    let authority = Authority::new();
    let mock = Mock::new(200, grok_wire::rotated(), 0).await;
    let cache = Cache::new(false).await;
    authority.backend.bind_cache(&cache.pool);
    let mut pending = Pending::new();
    refresh(&authority.runtime, &mock, &cache, &mut pending).await;
    let saved = authority
        .runtime
        .route_config()
        .snapshot()
        .document()
        .clone();
    drop(pending);
    let mut deleted = saved.clone();
    deleted.providers[0].credentials.clear();
    authority
        .runtime
        .commit_document(
            authority.runtime.route_config().snapshot().revision().id(),
            deleted,
            None,
        )
        .await
        .unwrap();
    authority
        .runtime
        .commit_document(
            authority.runtime.route_config().snapshot().revision().id(),
            saved,
            None,
        )
        .await
        .unwrap();
    let restarted = authority.restart().await;
    cache.allow_writes();
    refresh(&restarted, &mock, &cache, &mut Pending::new()).await;
    assert_eq!(cache.commits(), 0);
    assert_eq!(cache.value()["api_key"], "synthetic-build-token");
    assert_eq!(mock.count(), 1);
    mock.stop().await;
    cache.stop().await;
}

#[tokio::test]
async fn malformed_or_missing_publication_is_not_reconstructed_from_matching_tokens() {
    for corrupt in [true, false] {
        let authority = Authority::new();
        let mock = Mock::new(200, grok_wire::rotated(), 0).await;
        let cache = Cache::new(false).await;
        authority.backend.bind_cache(&cache.pool);
        let mut pending = Pending::new();
        refresh(&authority.runtime, &mock, &cache, &mut pending).await;
        drop(pending);
        let path = std::fs::read_dir(directory(&authority))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        if corrupt {
            let mut value: serde_json::Value =
                serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
            value["grant"]["identity"] = serde_json::json!("0".repeat(64));
            std::fs::write(path, serde_json::to_vec(&value).unwrap()).unwrap();
        } else {
            std::fs::remove_file(path).unwrap(); // The fixture root is under the explicit linshi TEMP.
        }
        let restarted = authority.restart().await;
        cache.allow_writes();
        refresh(&restarted, &mock, &cache, &mut Pending::new()).await;
        assert_eq!(cache.commits(), 0);
        assert_eq!(mock.count(), 1);
        mock.stop().await;
        cache.stop().await;
    }
}
