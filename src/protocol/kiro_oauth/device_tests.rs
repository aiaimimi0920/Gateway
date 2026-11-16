//! Tests live under the device owner to exercise time/state without exposing mutation APIs.
use super::super::{fixture::*, TEST_BODY_STARTED, TEST_ORIGIN};
use super::*;
use serde_json::{json, Value};

#[tokio::test]
async fn device_pending_slow_down_and_success_are_paced_single_steps() {
    let mock = Mock::new(vec![
        registration(),
        device(Some(1)),
        ResponseSpec::json(
            400,
            json!({"error":"authorization_pending","error_description":"private"}),
        ),
        ResponseSpec::json(400, json!({"error":"slow_down"})),
        tokens(),
    ])
    .await;
    let http = http();
    let client = register(&mock, &http).await;
    let mut challenge = begin(&mock, &client, &http).await;
    assert!(challenge.remaining() <= Duration::from_secs(600));
    assert!(tokio::time::timeout(
        Duration::from_millis(40),
        TEST_ORIGIN.scope(mock.origin.clone(), challenge.poll(&http))
    )
    .await
    .is_err());
    assert_eq!(mock.count(), 2, "pacing must happen before sending");
    let deadline = challenge.deadline;
    challenge.next_poll = Instant::now();
    assert!(matches!(
        TEST_ORIGIN
            .scope(mock.origin.clone(), challenge.poll(&http))
            .await
            .unwrap(),
        DevicePoll::Pending
    ));
    assert!(
        challenge
            .next_poll
            .saturating_duration_since(Instant::now())
            > Duration::from_millis(900)
    );
    challenge.next_poll = Instant::now();
    assert!(matches!(
        TEST_ORIGIN
            .scope(mock.origin.clone(), challenge.poll(&http))
            .await
            .unwrap(),
        DevicePoll::SlowDown
    ));
    assert_eq!(challenge.interval(), Duration::from_secs(6));
    assert_eq!(
        challenge.deadline, deadline,
        "slow_down cannot extend device expiry"
    );
    challenge.next_poll = Instant::now();
    assert!(matches!(
        TEST_ORIGIN
            .scope(mock.origin.clone(), challenge.poll(&http))
            .await
            .unwrap(),
        DevicePoll::Authorized(_)
    ));
    assert_eq!(
        challenge.poll(&http).await.unwrap_err(),
        OAuthError::Expired
    );
    let calls = mock.calls.lock().unwrap().clone();
    assert_eq!(calls.len(), 5);
    for call in &calls[2..] {
        let body: Value = serde_json::from_slice(&call.2).unwrap();
        assert_eq!(
            body,
            json!({"clientId":"synthetic-gateway-client","clientSecret":"synthetic-registered-secret",
            "grantType":"urn:ietf:params:oauth:grant-type:device_code","deviceCode":"synthetic-device-secret"})
        );
    }
    mock.stop().await;
}
#[tokio::test]
async fn device_denied_expired_and_invalid_success_are_terminal_without_retry() {
    for (reply, error) in [
        (
            ResponseSpec::json(400, json!({"error":"access_denied"})),
            OAuthError::AccessDenied,
        ),
        (
            ResponseSpec::json(400, json!({"error":"expired_token"})),
            OAuthError::Expired,
        ),
        (
            ResponseSpec::json(200, json!({"error":"authorization_pending"})),
            OAuthError::InvalidResponse,
        ),
        (
            ResponseSpec::json(401, json!({"error":"invalid_client"})),
            OAuthError::ReauthorizationRequired,
        ),
    ] {
        let mock = Mock::new(vec![registration(), device(Some(1)), reply]).await;
        let http = http();
        let client = register(&mock, &http).await;
        let mut challenge = begin(&mock, &client, &http).await;
        challenge.next_poll = Instant::now();
        assert_eq!(
            TEST_ORIGIN
                .scope(mock.origin.clone(), challenge.poll(&http))
                .await
                .unwrap_err(),
            error
        );
        assert_eq!(
            challenge.poll(&http).await.unwrap_err(),
            OAuthError::Expired
        );
        assert_eq!(mock.count(), 3);
        mock.stop().await;
    }
}
#[tokio::test]
async fn cancellation_after_send_retires_device_and_releases_the_body() {
    let mut hanging = tokens();
    hanging.body_delay = Duration::from_secs(60);
    let mock = Mock::new(vec![registration(), device(Some(1)), hanging]).await;
    let http = http();
    let client = register(&mock, &http).await;
    let mut challenge = begin(&mock, &client, &http).await;
    challenge.next_poll = Instant::now();
    mock.wait_drops(2).await;
    let reading = std::sync::Arc::new(tokio::sync::Notify::new());
    let mut pending = Box::pin(TEST_BODY_STARTED.scope(
        reading.clone(),
        TEST_ORIGIN.scope(mock.origin.clone(), challenge.poll(&http)),
    ));
    tokio::select! { _ = reading.notified() => {}, result = &mut pending => panic!("unexpected result: {result:?}") }
    drop(pending);
    mock.wait_drops(3).await;
    assert_eq!(
        challenge.poll(&http).await.unwrap_err(),
        OAuthError::Expired
    );
    assert_eq!(mock.count(), 3);
    mock.stop().await;
}
#[tokio::test]
async fn cancellation_before_response_headers_still_retires_the_sent_device_capability() {
    let mut delayed = tokens();
    delayed.header_delay = Duration::from_secs(60);
    let mock = Mock::new(vec![registration(), device(Some(1)), delayed]).await;
    let http = http();
    let client = register(&mock, &http).await;
    let mut challenge = begin(&mock, &client, &http).await;
    challenge.next_poll = Instant::now();
    let mut pending = Box::pin(TEST_ORIGIN.scope(mock.origin.clone(), challenge.poll(&http)));
    tokio::select! { _ = mock.wait_count(3) => {}, result = &mut pending => panic!("unexpected result: {result:?}") }
    drop(pending);
    assert_eq!(
        challenge.poll(&http).await.unwrap_err(),
        OAuthError::Expired
    );
    assert_eq!(mock.count(), 3);
    mock.stop().await;
}
#[tokio::test]
async fn cancelled_or_expired_device_never_sends_another_token_request() {
    let mock = Mock::new(vec![registration(), device(Some(1)), device(Some(1))]).await;
    let http = http();
    let client = register(&mock, &http).await;
    let mut cancelled = begin(&mock, &client, &http).await;
    cancelled.cancel();
    assert_eq!(
        cancelled.poll(&http).await.unwrap_err(),
        OAuthError::Expired
    );
    let mut expired = begin(&mock, &client, &http).await;
    expired.deadline = Instant::now();
    assert_eq!(expired.poll(&http).await.unwrap_err(), OAuthError::Expired);
    assert_eq!(mock.count(), 3);
    mock.stop().await;
}
#[tokio::test]
async fn device_reply_rejects_missing_lifetime_explicit_zero_and_foreign_login_urls() {
    let good: Value = serde_json::from_slice(&device(None).body).unwrap();
    let mut replies = Vec::new();
    let mut missing = good.clone();
    missing.as_object_mut().unwrap().remove("expiresIn");
    replies.push(missing);
    for (key, value) in [
        ("expiresIn", json!(0)),
        ("interval", json!(0)),
        ("verificationUri", json!("https://untrusted.invalid/")),
        (
            "verificationUriComplete",
            json!("https://device.sso.eu-west-1.amazonaws.com/"),
        ),
        ("deviceCode", json!("")),
        ("userCode", json!("with spaces")),
    ] {
        let mut reply = good.clone();
        reply[key] = value;
        replies.push(reply);
    }
    let mut specs = vec![registration()];
    specs.extend(
        replies
            .into_iter()
            .map(|reply| ResponseSpec::json(200, reply)),
    );
    let count = specs.len();
    let mock = Mock::new(specs).await;
    let http = http();
    let client = register(&mock, &http).await;
    for _ in 1..count {
        assert_eq!(
            TEST_ORIGIN
                .scope(mock.origin.clone(), client.begin_device(&http))
                .await
                .unwrap_err(),
            OAuthError::InvalidResponse
        );
    }
    assert_eq!(mock.count(), count);
    mock.stop().await;
}
#[tokio::test]
async fn delayed_device_response_does_not_restart_server_lifetime_after_read() {
    let mut reply: Value = serde_json::from_slice(&device(None).body).unwrap();
    reply["expiresIn"] = json!(1);
    let mut delayed = ResponseSpec::json(200, reply);
    delayed.body_delay = Duration::from_millis(1100);
    let mock = Mock::new(vec![registration(), delayed]).await;
    let http = http();
    let client = register(&mock, &http).await;
    assert_eq!(
        TEST_ORIGIN
            .scope(mock.origin.clone(), client.begin_device(&http))
            .await
            .unwrap_err(),
        OAuthError::Expired
    );
    assert_eq!(mock.count(), 2);
    mock.stop().await;
}
