//! Reopening the real file owner must not turn a prior send intent into permission to resend.
use super::{luma_tests::revision, *};
use crate::console::{
    ConsoleConfig, ConsoleConfigValues, RouteConfigPersistence, RouteConfigRuntime,
};

#[tokio::test]
async fn luma_grant_storage_reopen_preserves_single_claim_and_blocks_changed_material() {
    let initial = revision(Some("2020-01-01T00:00:00Z"));
    let root = std::env::temp_dir().join(format!("luma-grant-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let routes = root.join("routes.yaml");
    std::fs::write(&routes, serde_yaml::to_string(initial.document()).unwrap()).unwrap();
    let config = ConsoleConfig::from_values(ConsoleConfigValues {
        state_dir: Some(root.join("state")),
        routes_file: Some(routes),
        ..Default::default()
    })
    .unwrap();
    let runtime = RouteConfigRuntime::new_local(&config).unwrap();
    let snapshot = runtime.route_config().snapshot();
    let original =
        RouteConfigRedisRevision::new(snapshot.revision().clone(), snapshot.document().clone())
            .unwrap();
    let request = GrantRequest::from_revision(&original, "luma-mcp", "saved-mcp").unwrap();
    let store = RouteConfigPersistence::new(&config.state_dir, &config.routes_file).unwrap();
    let first = {
        let guard = store.try_writer_lock().unwrap();
        match store
            .reserve_refresh_grant_locked(&guard, &original, &request)
            .unwrap()
        {
            GrantReservation::Reserved(record) => record,
            other => panic!("expected first reservation, got {other:?}"),
        }
    };
    assert_eq!(first.phase(), GrantPhase::Sending);
    assert!(first.replay_for(&request).is_none());
    let locator = GrantLocator::from_record(&first).unwrap();
    drop(store);
    drop(runtime);
    let runtime = RouteConfigRuntime::new_local(&config).unwrap();
    assert_eq!(
        runtime.route_config().snapshot().revision().id(),
        original.metadata().id()
    );
    let reopened = RouteConfigPersistence::new(&config.state_dir, &config.routes_file).unwrap();
    {
        let guard = reopened.try_writer_lock().unwrap();
        assert_eq!(
            reopened
                .load_refresh_grant_locked(&guard, &locator)
                .unwrap(),
            Some(first.clone())
        );
        assert_eq!(
            reopened
                .reserve_refresh_grant_locked(&guard, &original, &request)
                .unwrap(),
            GrantReservation::Existing(first.clone())
        );
    }
    // A changed access token is not evidence that the same refresh token was never consumed.
    let mut changed = original.document().clone();
    changed.providers[0].credentials[0].api_key = Some("edited-access".into());
    let saved = runtime
        .commit_document(original.metadata().id(), changed.clone(), None)
        .await
        .unwrap();
    let successor = RouteConfigRedisRevision::new(saved.revision().clone(), changed).unwrap();
    let changed = GrantRequest::from_revision(&successor, "luma-mcp", "saved-mcp").unwrap();
    assert_eq!(request.family_id(), changed.family_id());
    assert_ne!(request.material_id(), changed.material_id());
    let guard = reopened.try_writer_lock().unwrap();
    assert_eq!(
        reopened
            .reserve_refresh_grant_locked(&guard, &successor, &changed)
            .unwrap(),
        GrantReservation::Blocked
    );
    assert_eq!(
        reopened
            .reserve_refresh_grant_locked(&guard, &original, &request)
            .unwrap(),
        GrantReservation::Stale
    );
    assert_eq!(
        reopened
            .load_refresh_grant_locked(&guard, &locator)
            .unwrap(),
        Some(first)
    );
}
