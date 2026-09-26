use super::tests::make_payload;
use super::*;
use std::time::Instant;

#[test]
fn extreme_remote_remaining_time_does_not_panic() {
    let config = FreeBuffRuntimeConfig::from_payload(&make_payload(), "z-ai/glm-5.1").unwrap();
    let snapshot = parse_free_session_snapshot(
        &json!({"status":"active","instanceId":"instance","remainingMs":u64::MAX}),
        &config,
    )
    .unwrap();
    assert_eq!(snapshot.active_instance_id(), Some("instance"));
}

#[test]
fn unrepresentable_synthetic_refresh_is_immediately_stale() {
    let mut config = FreeBuffRuntimeConfig::from_payload(&make_payload(), "z-ai/glm-5.1").unwrap();
    config.session_poll_interval = Duration::MAX;
    assert!(Instant::now()
        .checked_add(config.session_poll_interval)
        .is_none());
    let snapshot = build_synthetic_session_snapshot(
        FreeBuffWaitingRoomRejection::WaitingRoomQueued,
        None,
        "{}",
        &config,
    );
    assert!(!snapshot.is_fresh());
}

#[tokio::test]
async fn unrepresentable_poll_deadline_is_rejected_before_io() {
    let mut config = FreeBuffRuntimeConfig::from_payload(&make_payload(), "z-ai/glm-5.1").unwrap();
    config.base_url = "http://127.0.0.1:1".into();
    config.session_poll_timeout = Duration::MAX;
    let client = Client::builder().build().unwrap();
    let error = ensure_free_session(&client, &config).await.unwrap_err();
    assert_eq!(
        error.code.as_deref(),
        Some("freebuff_invalid_session_poll_timeout")
    );
    assert_eq!(error.http_status, Some(400));
}
