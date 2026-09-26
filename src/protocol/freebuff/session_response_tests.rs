use super::tests::make_payload;
use super::*;

#[test]
fn snapshot_wrappers_aliases_and_missing_status_preserve_wire_contract() {
    let config = FreeBuffRuntimeConfig::from_payload(&make_payload(), "z-ai/glm-5.1").unwrap();
    let snapshot = parse_free_session_snapshot(
        &json!({"session":{"status":" QUEUED ","claimed_instance_id":123,
            "queue_position":-3,"queue_depth":" 10 ","estimated_wait_millis":"1"},
            "data":{"status":"active"},"reason":" outer reason "}),
        &config,
    )
    .unwrap();
    assert_eq!(snapshot.state, FreeBuffSessionState::Queued);
    assert_eq!(snapshot.instance_id.as_deref(), Some("123"));
    assert_eq!(snapshot.queue_position, Some(0));
    assert_eq!(snapshot.queue_depth, Some(10));
    assert_eq!(snapshot.estimated_wait_ms, Some(1));
    assert_eq!(snapshot.message.as_deref(), Some("outer reason"));
    assert_eq!(snapshot.retry_delay_ms(999), 250);
    assert!(snapshot.active_instance_id().is_none());
    assert_eq!(
        parse_free_session_snapshot(&json!({}), &config)
            .unwrap()
            .state,
        FreeBuffSessionState::None
    );
    for (status, state) in [
        ("disabled", FreeBuffSessionState::Disabled),
        ("ended", FreeBuffSessionState::Draining),
        ("superseded", FreeBuffSessionState::Superseded),
    ] {
        assert_eq!(
            parse_free_session_snapshot(&json!({"data":{"status":status}}), &config)
                .unwrap()
                .state,
            state
        );
    }
}

#[test]
fn snapshot_errors_and_active_identity_remain_explicit() {
    let config = FreeBuffRuntimeConfig::from_payload(&make_payload(), "z-ai/glm-5.1").unwrap();
    for (body, code) in [
        (
            json!({"status":"expired"}),
            "freebuff_unknown_session_state",
        ),
        (json!({"status":"active"}), "freebuff_missing_instance_id"),
    ] {
        let error = parse_free_session_snapshot(&body, &config).unwrap_err();
        assert_eq!(error.code.as_deref(), Some(code));
        assert_eq!(error.http_status, Some(500));
    }
    let snapshot = parse_free_session_snapshot(
        &json!({"status":"active","instanceId":" instance ","remainingMs":0}),
        &config,
    )
    .unwrap();
    assert_eq!(snapshot.active_instance_id(), Some("instance"));
    assert!(snapshot.is_fresh());
}

#[test]
fn synthetic_rejections_and_error_statuses_keep_existing_mapping() {
    let config = FreeBuffRuntimeConfig::from_payload(&make_payload(), "z-ai/glm-5.1").unwrap();
    for (rejection, state, status, code) in [
        (
            FreeBuffWaitingRoomRejection::MissingInstance,
            FreeBuffSessionState::None,
            400,
            "freebuff_update_required",
        ),
        (
            FreeBuffWaitingRoomRejection::WaitingRoomRequired,
            FreeBuffSessionState::None,
            503,
            "waiting_room_required",
        ),
        (
            FreeBuffWaitingRoomRejection::WaitingRoomQueued,
            FreeBuffSessionState::Queued,
            429,
            "waiting_room_queued",
        ),
        (
            FreeBuffWaitingRoomRejection::SessionSuperseded,
            FreeBuffSessionState::Superseded,
            409,
            "session_superseded",
        ),
        (
            FreeBuffWaitingRoomRejection::SessionExpired,
            FreeBuffSessionState::Expired,
            429,
            "session_expired",
        ),
    ] {
        let body = r#"{"error":{"message":" nested "},"message":"outer"}"#;
        let snapshot = build_synthetic_session_snapshot(rejection, Some("instance"), body, &config);
        assert_eq!(snapshot.state, state);
        assert_eq!(snapshot.instance_id.as_deref(), Some("instance"));
        assert_eq!(snapshot.message.as_deref(), Some("nested"));
        let error = classify_waiting_room_error(rejection, body, &config);
        assert_eq!(error.http_status, Some(status));
        assert_eq!(error.code.as_deref(), Some(code));
        assert_eq!(error.message, "nested");
    }
    assert_eq!(
        classify_waiting_room_rejection(429, "ordinary rate limit"),
        None
    );
    assert_eq!(
        classify_waiting_room_rejection(500, "waiting_room_queued"),
        None
    );
}
