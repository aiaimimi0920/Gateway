//! A read-only Worker gains only typed refresh persistence, never general mutations.
use super::{authority_fixture::Authority, cache_wire::Cache, grok_wire::Mock, *};
use serde_json::json;

pub(super) async fn refresh(
    authority: &Authority,
    mock: &Mock,
    cache: &Cache,
    pending: &mut Pending,
) {
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

pub(super) fn reply() -> serde_json::Value {
    let mut reply = grok_wire::rotated();
    reply["scope"] = json!("profile");
    reply
}

#[tokio::test]
async fn worker_refresh_saves_authority_and_cache_without_opening_general_writes() {
    let authority = Authority::with_writable(false);
    assert!(authority.runtime.coordinator().is_none());
    let snapshot = authority.runtime.route_config().snapshot();
    let before = serde_json::to_value(snapshot.document()).unwrap();
    for automation in [false, true] {
        let result = if automation {
            authority
                .runtime
                .commit_automation_document(
                    snapshot.revision().id(),
                    snapshot.document().clone(),
                    None,
                )
                .await
        } else {
            authority
                .runtime
                .commit_document(snapshot.revision().id(), snapshot.document().clone(), None)
                .await
        };
        assert_eq!(result.unwrap_err().code(), "console_mutation_not_supported");
    }
    let cache = Cache::new(true).await;
    authority.backend.bind_cache(&cache.pool);
    let mock = Mock::new(200, reply(), 0).await;
    let mut pending = Pending::new();
    refresh(&authority, &mock, &cache, &mut pending).await;
    assert_eq!(
        authority.saved().api_key.as_deref(),
        Some("rotated-build-access")
    );
    assert!(pending.is_empty());
    assert!(authority.runtime.coordinator().is_none());
    server_authority::assert_cache_matches_authority(&authority, &cache);
    let mut expected = before;
    let target = &mut expected["providers"][0]["credentials"][0];
    let saved = serde_json::to_value(authority.saved()).unwrap();
    for field in [
        "api_key",
        "refresh_token",
        "token_expires_in_secs",
        "expires_at",
        "grok_oauth",
    ] {
        target[field] = saved[field].clone();
    }
    assert_eq!(
        serde_json::to_value(authority.active().document()).unwrap(),
        expected
    );
    assert_eq!(mock.count(), 1);
    mock.stop().await;
    cache.stop().await;
}

#[tokio::test]
async fn worker_cache_outage_retries_same_rotation_after_authority_commit() {
    let authority = Authority::with_writable(false);
    let cache = Cache::new(false).await;
    authority.backend.bind_cache(&cache.pool);
    let mock = Mock::new(200, reply(), 0).await;
    let mut pending = Pending::new();
    refresh(&authority, &mock, &cache, &mut pending).await;
    assert_eq!(
        authority.saved().api_key.as_deref(),
        Some("rotated-build-access")
    );
    assert_eq!(pending.len(), 1);
    let revision = authority.active().metadata().id().to_owned();
    cache.allow_writes();
    refresh(&authority, &mock, &cache, &mut pending).await;
    assert!(pending.is_empty());
    assert_eq!(mock.count(), 1);
    assert_eq!(authority.backend.activations(), 1);
    assert_eq!(authority.active().metadata().id(), revision);
    server_authority::assert_cache_matches_authority(&authority, &cache);
    mock.stop().await;
    cache.stop().await;
}
