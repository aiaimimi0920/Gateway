//! Authorization context and device-state invariants; all potential sends target loopback.
use super::*;
use std::time::Duration;

#[test]
fn pkce_matches_rfc7636_vector() {
    let mut auth = Authorization::new(&client(), "http://127.0.0.1:1234/callback").unwrap();
    auth.verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk".into();
    let url = url::Url::parse(&auth.authorize_url()).unwrap();
    assert_eq!(
        url.query_pairs()
            .find(|(k, _)| k == "code_challenge")
            .unwrap()
            .1,
        "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
    );
}
#[tokio::test]
async fn grants_reject_client_principal_or_issuer_rebinding_before_network() {
    let fixture =
        wire::spawn_fixture(axum::http::StatusCode::OK, json!({"access_token":"access"})).await;
    let auth = Authorization::new(&fixture.client, "http://127.0.0.1:1234/callback").unwrap();
    let changed = fixture
        .client
        .clone()
        .with_principal(Principal::new("Team", "other-team").unwrap());
    assert_eq!(
        changed
            .exchange_code(&wire::http(), &auth, "private-code")
            .await
            .unwrap_err(),
        OAuthError::InvalidInput
    );
    let mut device = DeviceChallenge::parse(device_body(30, 5), &client()).unwrap();
    assert_eq!(
        fixture
            .client
            .poll_device(&wire::http(), &mut device)
            .await
            .unwrap_err(),
        OAuthError::InvalidInput
    );
    assert!(fixture.calls.lock().unwrap().is_empty());
}
#[tokio::test]
async fn device_rebinding_of_principal_or_scope_never_posts() {
    for principal_change in [true, false] {
        let fixture =
            wire::spawn_fixture(axum::http::StatusCode::OK, json!({"access_token":"access"})).await;
        let changed = if principal_change {
            fixture
                .client
                .clone()
                .with_principal(Principal::new("Team", "different-team").unwrap())
        } else {
            OAuthClient::new(fixture.client.client_id(), &["openid"])
                .unwrap()
                .test_issuer(&fixture.client.issuer)
        };
        let mut device = DeviceChallenge::parse(device_body(60, 1), &fixture.client).unwrap();
        device.next_poll = tokio::time::Instant::now();
        assert_eq!(
            changed.poll_device(&wire::http(), &mut device).await,
            Err(OAuthError::InvalidInput)
        );
        assert!(fixture.calls.lock().unwrap().is_empty());
        assert!(!device.terminal);
    }
}
#[tokio::test(start_paused = true)]
async fn shorter_device_expiry_and_cancelled_wait_do_not_send_a_poll() {
    let fixture =
        wire::spawn_fixture(axum::http::StatusCode::OK, json!({"access_token":"access"})).await;
    let mut device = DeviceChallenge::parse(device_body(1, 5), &fixture.client).unwrap();
    assert_eq!(
        fixture
            .client
            .poll_device(&wire::http(), &mut device)
            .await
            .unwrap_err(),
        OAuthError::Expired
    );
    let mut device = DeviceChallenge::parse(device_body(60, 5), &fixture.client).unwrap();
    assert!(tokio::time::timeout(
        Duration::from_millis(1),
        fixture.client.poll_device(&wire::http(), &mut device)
    )
    .await
    .is_err());
    assert!(!device.terminal);
    assert!(fixture.calls.lock().unwrap().is_empty());
}
#[tokio::test]
async fn terminal_device_reply_cannot_be_polled_again() {
    for (status, body) in [
        (
            axum::http::StatusCode::OK,
            json!({"access_token":"new-access"}),
        ),
        (
            axum::http::StatusCode::BAD_REQUEST,
            json!({"error":"access_denied"}),
        ),
    ] {
        let fixture = wire::spawn_fixture(status, body).await;
        let mut device = DeviceChallenge::parse(device_body(60, 1), &fixture.client).unwrap();
        device.next_poll = tokio::time::Instant::now();
        let result = fixture.client.poll_device(&wire::http(), &mut device).await;
        if status.is_success() {
            assert!(matches!(result, Ok(DevicePoll::Authorized(_))));
        } else {
            assert_eq!(result.unwrap_err(), OAuthError::AccessDenied);
        }
        assert_eq!(
            fixture
                .client
                .poll_device(&wire::http(), &mut device)
                .await
                .unwrap_err(),
            OAuthError::Expired
        );
        assert_eq!(fixture.calls.lock().unwrap().len(), 1);
    }
}
fn device_body(ttl: u64, interval: u64) -> Value {
    json!({"device_code":"private-device","user_code":"ABCD","verification_uri":"https://accounts.x.ai/device","expires_in":ttl,"interval":interval})
}
