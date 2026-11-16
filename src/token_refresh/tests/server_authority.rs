//! A Server rotation must save normal configuration authority and its derived cache before completion.
use super::{
    authority_fixture::Authority,
    cache_wire::Cache,
    grok_wire::{rotated, Mock},
    *,
};
use serde_json::json;

async fn refresh(authority: &Authority, mock: &Mock, cache: &Cache, pending: &mut Pending) {
    super::super::grok::TEST_ISSUER
        .scope(
            mock.issuer.clone(),
            refresh_once(
                authority.runtime.route_config(),
                &client(),
                Some(&authority.runtime),
                Some(&cache.pool),
                pending,
                Duration::from_secs(300),
                None,
            ),
        )
        .await;
}
fn narrowed() -> serde_json::Value {
    let mut reply = rotated();
    reply["scope"] = json!("profile");
    reply
}

pub(super) fn assert_cache_matches_authority(authority: &Authority, cache: &Cache) {
    let saved = authority.saved();
    let value = cache.value();
    assert_eq!(value["api_key"], json!(saved.api_key));
    assert_eq!(value["expires_at"], json!(saved.expires_at));
    assert_eq!(
        value["account_payload"]["refresh_token"],
        json!(saved.refresh_token)
    );
    assert_eq!(
        value["account_payload"]["grok_oauth"],
        json!(saved.grok_oauth)
    );
    assert_eq!(
        value["account_payload"]["expires_at"],
        json!(saved.expires_at)
    );
    assert_eq!(
        value["account_payload"]["token_expires_in_secs"],
        json!(saved.token_expires_in_secs)
    );
    assert_eq!(value["quota_remaining_tokens"], 42);
}

#[tokio::test]
async fn server_normal_authority_and_cache_persist_rotation_then_recover_stale_bootstrap_twice() {
    let authority = Authority::new();
    let cache = Cache::new(true).await;
    authority.backend.bind_cache(&cache.pool);
    let mock = Mock::new(200, narrowed(), 0).await;
    let mut pending = Pending::new();
    refresh(&authority, &mock, &cache, &mut pending).await;
    assert!(pending.is_empty());
    assert_eq!(
        cache.commits(),
        1,
        "authority commit must not skip the derived cache"
    );
    let saved = authority.saved();
    assert_cache_matches_authority(&authority, &cache);
    assert_eq!(
        saved.refresh_token.as_deref(),
        Some("rotated-build-refresh")
    );
    assert_eq!(
        serde_json::to_value(&saved.grok_oauth).unwrap()["granted_scopes"],
        json!(["profile"])
    );
    assert_eq!(
        authority.active().document().providers[0].credentials[0].api_key,
        saved.api_key
    );
    assert_eq!(
        authority.active().metadata().actor(),
        crate::console::revision::RevisionActor::CredentialPoolAutomation
    );
    let mirror: RouteConfigYaml =
        serde_yaml::from_str(&std::fs::read_to_string(&authority.routes).unwrap()).unwrap();
    assert_eq!(
        mirror.providers[0].credentials[0].refresh_token,
        saved.refresh_token
    );
    let records = crate::console::journal::TransactionJournal::new(authority.persistence.clone())
        .load_transactions()
        .unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(
        records[0].phase(),
        crate::console::TransactionPhase::Committed
    );
    let archive = authority
        .persistence
        .load_revision(records[0].new_revision())
        .unwrap()
        .unwrap();
    let archived = &archive.document().providers[0].credentials[0];
    assert_eq!(archived.api_key, saved.api_key);
    assert_eq!(archived.refresh_token, saved.refresh_token);
    assert_eq!(archived.grok_oauth, saved.grok_oauth);
    assert_eq!(archived.expires_at, saved.expires_at);
    for _ in 0..2 {
        let restarted = authority.restart().await;
        let snapshot = restarted.route_config().snapshot();
        let restored = &snapshot.document().providers[0].credentials[0];
        assert_eq!(restored.api_key, saved.api_key);
        assert_eq!(restored.refresh_token, saved.refresh_token);
        assert_eq!(restored.grok_oauth, saved.grok_oauth);
        assert_eq!(restored.expires_at, saved.expires_at);
        assert_eq!(restored.token_expires_in_secs, saved.token_expires_in_secs);
        assert_eq!(snapshot.revision().id(), authority.active().metadata().id());
    }
    assert_eq!(cache.value()["quota_remaining_tokens"], 42);
    assert_eq!(mock.count(), 1);
    mock.stop().await;
    cache.stop().await;
}

#[tokio::test]
async fn server_authority_commit_then_cache_outage_retains_pending_and_retries_cache_only() {
    let authority = Authority::new();
    let cache = Cache::new(false).await;
    authority.backend.bind_cache(&cache.pool);
    let mock = Mock::new(200, narrowed(), 0).await;
    let mut pending = Pending::new();
    refresh(&authority, &mock, &cache, &mut pending).await;
    assert_eq!(
        pending.len(),
        1,
        "authority success is not completion while cache is pending"
    );
    let revision = authority.active().metadata().id().to_owned();
    assert_eq!(
        authority.saved().api_key.as_deref(),
        Some("rotated-build-access")
    );
    assert!(Arc::ptr_eq(
        &pending.values().next().unwrap().as_result().unwrap().owner,
        &server_policy::owner(authority.runtime.route_config())
    ));
    // Even an independent scheduler with an unlimited margin cannot bypass the pending ticket lock.
    let mut independent = Pending::new();
    super::super::grok::TEST_ISSUER
        .scope(
            mock.issuer.clone(),
            refresh_once(
                authority.runtime.route_config(),
                &client(),
                Some(&authority.runtime),
                Some(&cache.pool),
                &mut independent,
                Duration::MAX,
                None,
            ),
        )
        .await;
    assert!(independent.is_empty());
    refresh(&authority, &mock, &cache, &mut pending).await;
    assert_eq!(mock.count(), 1);
    assert_eq!(authority.active().metadata().id(), revision);
    cache.allow_writes();
    refresh(&authority, &mock, &cache, &mut pending).await;
    assert!(
        pending.is_empty(),
        "the scheduler must release its own completed ticket"
    );
    assert_eq!(cache.commits(), 1);
    assert_cache_matches_authority(&authority, &cache);
    assert_eq!(authority.active().metadata().id(), revision);
    assert_eq!(mock.count(), 1);
    mock.stop().await;
    cache.stop().await;
}

#[tokio::test]
async fn server_authority_writer_conflict_does_not_write_cache_or_repeat_grant() {
    let authority = Authority::new();
    let cache = Cache::new(true).await;
    authority.backend.bind_cache(&cache.pool);
    let mock = Mock::new(200, narrowed(), 100).await;
    let mut pending = Pending::new();
    let guard = {
        let task = refresh(&authority, &mock, &cache, &mut pending);
        tokio::pin!(task);
        let reached = async {
            tokio::time::timeout(Duration::from_secs(3), async {
                while mock.count() == 0 {
                    tokio::task::yield_now().await;
                }
            })
            .await
            .unwrap();
            authority.persistence.try_writer_lock().unwrap()
        };
        let guard = tokio::select! {
            guard = reached => guard,
            _ = &mut task => panic!("refresh finished before the writer-failure window"),
        };
        task.await;
        guard
    };
    assert_eq!(pending.len(), 1);
    assert_eq!(cache.commits(), 0, "cache must not get ahead of authority");
    assert_eq!(
        authority.saved().api_key.as_deref(),
        Some("synthetic-build-token")
    );
    drop(guard);
    refresh(&authority, &mock, &cache, &mut pending).await;
    assert!(pending.is_empty());
    assert_eq!(mock.count(), 1);
    assert_eq!(cache.commits(), 1);
    assert_cache_matches_authority(&authority, &cache);
    mock.stop().await;
    cache.stop().await;
}

#[tokio::test]
async fn server_reauthorization_while_cache_pending_discards_old_writeback() {
    let authority = Authority::new();
    let cache = Cache::new(false).await;
    authority.backend.bind_cache(&cache.pool);
    let mock = Mock::new(200, narrowed(), 0).await;
    let mut pending = Pending::new();
    refresh(&authority, &mock, &cache, &mut pending).await;
    assert_eq!(pending.len(), 1);
    let snapshot = authority.runtime.route_config().snapshot();
    let mut document = snapshot.document().clone();
    document.providers[0].credentials[0].api_key = Some("explicit-new-access".into());
    document.providers[0].credentials[0].refresh_token = Some("explicit-new-refresh".into());
    authority
        .runtime
        .commit_document(snapshot.revision().id(), document, None)
        .await
        .unwrap();
    cache.allow_writes();
    refresh(&authority, &mock, &cache, &mut pending).await;
    assert!(pending.is_empty());
    assert_eq!(mock.count(), 1);
    assert_eq!(cache.commits(), 0);
    assert_eq!(
        authority.saved().api_key.as_deref(),
        Some("explicit-new-access")
    );
    assert_eq!(cache.value()["api_key"], "synthetic-build-token");
    mock.stop().await;
    cache.stop().await;
}
