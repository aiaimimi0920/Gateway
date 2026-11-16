//! Real file storage retains an unusable successor without releasing the original send intent.
use super::{luma_tests::revision, *};
use crate::console::{
    ConsoleConfig, ConsoleConfigValues, RouteConfigPersistence, RouteConfigRuntime,
};
use crate::protocol::luma_oauth::ReceivedRefresh;

#[tokio::test]
async fn luma_received_storage_reopens_receipt_after_authority_changes_without_releasing_grant() {
    let initial = revision(None);
    let root = std::env::temp_dir().join(format!("luma-receipt-{}", uuid::Uuid::new_v4()));
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
    let sending = {
        let guard = store.try_writer_lock().unwrap();
        match store
            .reserve_refresh_grant_locked(&guard, &original, &request)
            .unwrap()
        {
            GrantReservation::Reserved(record) => record,
            other => panic!("expected reservation: {other:?}"),
        }
    };
    let context = request.material.luma_oauth.as_ref().unwrap();
    let tokens = ReceivedRefresh::capture(
        br#"{"refresh_token":"received-successor","expires_in":false}"#,
        time::OffsetDateTime::now_utc(),
        context,
        &request.material.refresh,
    )
    .unwrap();
    assert!(tokens
        .interpret_successor(context, &request.material.refresh)
        .is_err());
    let receipt = GrantReceipt::LumaOauth { tokens };
    // Revoked authority prevents publication, not recording a response to a consumed request.
    let mut changed = original.document().clone();
    changed.providers[0].credentials[0].api_key = Some("changed-access".into());
    runtime
        .commit_document(original.metadata().id(), changed, None)
        .await
        .unwrap();
    let saved = {
        let guard = store.try_writer_lock().unwrap();
        let saved = store
            .store_refresh_grant_receipt_locked(&guard, &sending, &receipt)
            .unwrap();
        assert_eq!(
            store
                .store_refresh_grant_receipt_locked(&guard, &sending, &receipt)
                .unwrap(),
            saved
        );
        saved
    };
    assert_eq!(saved.phase(), GrantPhase::Sending);
    assert!(saved.result().is_none());
    assert!(saved.replay_for(&request).is_none());
    assert!(saved.receipt_for(&request).is_some());
    let locator = GrantLocator::from_record(&sending).unwrap();
    assert!(locator.matches(&saved));
    drop(store);
    drop(runtime);
    let reopened = RouteConfigRuntime::new_local(&config).unwrap();
    let snapshot = reopened.route_config().snapshot();
    let current =
        RouteConfigRedisRevision::new(snapshot.revision().clone(), snapshot.document().clone())
            .unwrap();
    let current_request = GrantRequest::from_revision(&current, "luma-mcp", "saved-mcp").unwrap();
    let store = RouteConfigPersistence::new(&config.state_dir, &config.routes_file).unwrap();
    let guard = store.try_writer_lock().unwrap();
    let restored = store
        .load_refresh_grant_locked(&guard, &locator)
        .unwrap()
        .unwrap();
    assert_eq!(restored, saved);
    assert!(restored.receipt_for(&current_request).is_none());
    assert_eq!(
        store
            .reserve_refresh_grant_locked(&guard, &current, &current_request)
            .unwrap(),
        GrantReservation::Blocked
    );
    assert!(store
        .mark_refresh_grant_published_locked(&guard, &current, &restored)
        .is_err());
    let other = GrantReceipt::LumaOauth {
        tokens: ReceivedRefresh::capture(
            b"{}",
            time::OffsetDateTime::now_utc(),
            context,
            &request.material.refresh,
        )
        .unwrap(),
    };
    assert!(store
        .store_refresh_grant_receipt_locked(&guard, &restored, &other)
        .is_err());
}
