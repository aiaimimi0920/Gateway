use super::*;
use crate::protocol::freebuff::session_response::parse_free_session_snapshot;
use crate::protocol::freebuff::tests::make_payload;
use std::future::Future;
use std::task::Poll;

fn config() -> FreeBuffRuntimeConfig {
    let mut config = FreeBuffRuntimeConfig::from_payload(&make_payload(), "z-ai/glm-5.1").unwrap();
    config.session_bucket_key = "observation".into();
    config
}

fn set(state: &mut FreeBuffSessionBucket, id: &str, status: &str) {
    state.set_snapshot(Some(
        parse_free_session_snapshot(
            &serde_json::json!({"status": status, "instanceId": id}),
            &config(),
        )
        .unwrap(),
    ));
}

#[tokio::test]
async fn delayed_rejection_rechecks_observation_after_waiting_for_lock() {
    let registry = SessionRegistry::new(4);
    let config = config();
    let bucket = registry.get_or_insert(&config.session_bucket_key).unwrap();
    let mut state = bucket.lock().await;
    set(&mut state, "A", "active");
    let observation = SessionObservation::capture(&bucket, &state, Some("A".into()));
    let mut rejection = Box::pin(registry.record_rejection(
        &config,
        &observation,
        FreeBuffWaitingRoomRejection::SessionExpired,
        "{}",
    ));
    std::future::poll_fn(|cx| {
        assert!(rejection.as_mut().poll(cx).is_pending());
        Poll::Ready(())
    })
    .await;
    set(&mut state, "B", "active");
    drop(state);
    rejection.await;
    let state = bucket.lock().await;
    assert_eq!(
        state.snapshot.as_ref().unwrap().instance_id.as_deref(),
        Some("B")
    );
    assert_eq!(
        state.snapshot.as_ref().unwrap().state,
        FreeBuffSessionState::Active
    );
}

#[tokio::test]
async fn new_observation_of_same_instance_rejects_old_response() {
    let registry = SessionRegistry::new(4);
    let config = config();
    let bucket = registry.get_or_insert(&config.session_bucket_key).unwrap();
    let mut state = bucket.lock().await;
    set(&mut state, "same", "active");
    let old = SessionObservation::capture(&bucket, &state, Some("same".into()));
    set(&mut state, "same", "active");
    drop(state);
    registry
        .record_rejection(
            &config,
            &old,
            FreeBuffWaitingRoomRejection::WaitingRoomQueued,
            "{}",
        )
        .await;
    assert_eq!(
        bucket.lock().await.snapshot.as_ref().unwrap().state,
        FreeBuffSessionState::Active
    );
}

#[tokio::test]
async fn first_rejection_advances_revision_for_other_requests_on_same_snapshot() {
    let registry = SessionRegistry::new(4);
    let config = config();
    let bucket = registry.get_or_insert(&config.session_bucket_key).unwrap();
    let mut state = bucket.lock().await;
    set(&mut state, "same", "active");
    let observation = SessionObservation::capture(&bucket, &state, Some("same".into()));
    drop(state);
    registry
        .record_rejection(
            &config,
            &observation,
            FreeBuffWaitingRoomRejection::WaitingRoomQueued,
            "{}",
        )
        .await;
    registry
        .record_rejection(
            &config,
            &observation,
            FreeBuffWaitingRoomRejection::SessionExpired,
            "{}",
        )
        .await;
    assert_eq!(
        bucket.lock().await.snapshot.as_ref().unwrap().state,
        FreeBuffSessionState::Queued
    );
}

#[tokio::test]
async fn unidentified_or_unobserved_rejection_cannot_erase_identified_claim() {
    for status in ["active", "queued", "ended"] {
        let registry = SessionRegistry::new(4);
        let config = config();
        let bucket = registry.get_or_insert(&config.session_bucket_key).unwrap();
        let mut state = bucket.lock().await;
        set(&mut state, "B", status);
        let unidentified = SessionObservation::capture(&bucket, &state, None);
        drop(state);
        for observation in [SessionObservation::default(), unidentified] {
            registry
                .record_rejection(
                    &config,
                    &observation,
                    FreeBuffWaitingRoomRejection::MissingInstance,
                    "{}",
                )
                .await;
            assert_eq!(
                bucket
                    .lock()
                    .await
                    .snapshot
                    .as_ref()
                    .unwrap()
                    .instance_id
                    .as_deref(),
                Some("B")
            );
        }
    }
}

#[tokio::test]
async fn old_bucket_observation_cannot_mutate_replacement() {
    let registry = SessionRegistry::new(4);
    let config = config();
    let old = registry.get_or_insert(&config.session_bucket_key).unwrap();
    let state = old.lock().await;
    let observation = SessionObservation::capture(&old, &state, None);
    drop(state);
    registry.buckets.remove(&config.session_bucket_key);
    let new = registry.get_or_insert(&config.session_bucket_key).unwrap();
    registry
        .record_rejection(
            &config,
            &observation,
            FreeBuffWaitingRoomRejection::WaitingRoomRequired,
            "{}",
        )
        .await;
    assert!(new.lock().await.snapshot.is_none());
}

#[tokio::test]
async fn unchanged_empty_observation_accepts_rejection() {
    let registry = SessionRegistry::new(1);
    let config = config();
    let bucket = registry.get_or_insert(&config.session_bucket_key).unwrap();
    let state = bucket.lock().await;
    let observation = SessionObservation::capture(&bucket, &state, None);
    drop(state);
    registry
        .record_rejection(
            &config,
            &observation,
            FreeBuffWaitingRoomRejection::WaitingRoomRequired,
            "{}",
        )
        .await;
    assert!(bucket.lock().await.snapshot.is_some());
}

#[tokio::test]
async fn cleared_snapshot_does_not_reuse_empty_revision() {
    let registry = SessionRegistry::new(1);
    let config = config();
    let bucket = registry.get_or_insert(&config.session_bucket_key).unwrap();
    let mut state = bucket.lock().await;
    let observation = SessionObservation::capture(&bucket, &state, None);
    set(&mut state, "A", "active");
    state.set_snapshot(None);
    drop(state);
    registry
        .record_rejection(
            &config,
            &observation,
            FreeBuffWaitingRoomRejection::WaitingRoomRequired,
            "{}",
        )
        .await;
    assert!(bucket.lock().await.snapshot.is_none());
}
