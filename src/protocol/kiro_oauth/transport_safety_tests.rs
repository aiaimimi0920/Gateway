//! Real HTTP decoded budgets, redirects, cancellation and registration expiry; synthetic only.
use super::{fixture::*, *};
use serde_json::{json, Value};
use std::time::Duration;

async fn post(mock: &Mock, budget: Duration) -> Result<Reply, OAuthError> {
    TEST_ORIGIN
        .scope(
            mock.origin.clone(),
            transport::post(
                &http(),
                &Directory::builder_id(),
                "/token",
                json!({"synthetic":"private-material"}),
                budget,
            ),
        )
        .await
}
#[tokio::test]
async fn redirects_are_never_followed_or_replayed() {
    let trap = Mock::new(vec![]).await;
    for status in [301, 302, 303, 307, 308] {
        let mut reply = tokens();
        reply.status = status;
        reply.headers.insert(
            "location",
            format!("{}/token", trap.origin).parse().unwrap(),
        );
        let mock = Mock::new(vec![reply]).await;
        assert_eq!(
            post(&mock, Duration::from_secs(2)).await.err().unwrap(),
            OAuthError::RedirectRejected
        );
        assert_eq!(mock.count(), 1);
        assert_eq!(trap.count(), 0);
        mock.stop().await;
    }
    trap.stop().await;
}
#[tokio::test]
async fn decoded_size_limit_covers_chunked_and_compressed_bodies() {
    let mut plain = tokens();
    plain.body = vec![b'x'; 65_537];
    // Same Python stdlib synthetic gzip fixture as Accio's decoded-byte contract: 97 wire bytes.
    let mut compressed = tokens();
    compressed.body = hex::decode("1f8b080000000000020aedc1010100000082a0dbceaf2140010000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000c00d81d82a9c01000100").unwrap();
    compressed
        .headers
        .insert("content-encoding", "gzip".parse().unwrap());
    for reply in [plain, compressed] {
        let mock = Mock::new(vec![reply]).await;
        assert_eq!(
            post(&mock, Duration::from_secs(2)).await.err().unwrap(),
            OAuthError::ResponseTooLarge
        );
        assert_eq!(mock.count(), 1);
        mock.stop().await;
    }
}
#[tokio::test]
async fn send_and_read_share_one_deadline_and_drop_cancels_the_transport() {
    for (headers, body) in [(120, 0), (0, 120), (40, 80)] {
        let mut reply = tokens();
        reply.header_delay = Duration::from_millis(headers);
        reply.body_delay = Duration::from_millis(body);
        let mock = Mock::new(vec![reply]).await;
        assert_eq!(
            post(&mock, Duration::from_millis(70)).await.err().unwrap(),
            OAuthError::Timeout
        );
        assert_eq!(mock.count(), 1);
        mock.stop().await;
    }
    let mut reply = tokens();
    reply.body_delay = Duration::from_secs(60);
    let mock = Mock::new(vec![reply]).await;
    let reading = std::sync::Arc::new(tokio::sync::Notify::new());
    let mut pending =
        Box::pin(TEST_BODY_STARTED.scope(reading.clone(), post(&mock, Duration::from_secs(15))));
    tokio::select! { _ = reading.notified() => {}, result = &mut pending => panic!("unexpected result: {:?}", result.err()) }
    drop(pending);
    mock.wait_drops(1).await;
    assert_eq!(mock.count(), 1);
    mock.stop().await;
}
#[tokio::test]
async fn duplicate_success_fields_and_private_error_bodies_do_not_produce_material() {
    let mut duplicated = registration();
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    duplicated.body = format!("{{\"clientId\":\"a\",\"clientId\":\"b\",\"clientSecret\":\"private\",\"clientIdIssuedAt\":{now},\"clientSecretExpiresAt\":{}}}", now+3600).into_bytes();
    let mock = Mock::new(vec![duplicated]).await;
    assert_eq!(
        TEST_ORIGIN
            .scope(
                mock.origin.clone(),
                ClientRegistration::register(
                    &http(),
                    Directory::builder_id(),
                    &[Scope::Conversations]
                )
            )
            .await
            .unwrap_err(),
        OAuthError::InvalidResponse
    );
    mock.stop().await;
    for status in [400, 401, 403, 429, 500] {
        let mock = Mock::new(vec![
            registration(),
            ResponseSpec::json(
                status,
                json!({"error":"invalid_grant","error_description":"private-sensitive-material"}),
            ),
        ])
        .await;
        let http = http();
        let client = register(&mock, &http).await;
        let error = TEST_ORIGIN
            .scope(
                mock.origin.clone(),
                client
                    .refresh_grant("synthetic-refresh")
                    .unwrap()
                    .execute(&http),
            )
            .await
            .unwrap_err();
        assert!(!format!("{error:?} {error}").contains("private-sensitive-material"));
        assert_eq!(mock.count(), 2);
        mock.stop().await;
    }
}
#[tokio::test]
async fn registered_client_expiry_is_not_restarted_or_reused_for_refresh() {
    let mut value: Value = serde_json::from_slice(&registration().body).unwrap();
    value["clientSecretExpiresAt"] = json!(time::OffsetDateTime::now_utc().unix_timestamp() + 1);
    let mut reply = ResponseSpec::json(200, value);
    reply.body_delay = Duration::from_millis(1200);
    let mock = Mock::new(vec![reply]).await;
    assert_eq!(
        TEST_ORIGIN
            .scope(
                mock.origin.clone(),
                ClientRegistration::register(
                    &http(),
                    Directory::builder_id(),
                    &[Scope::Conversations]
                )
            )
            .await
            .unwrap_err(),
        OAuthError::RegistrationExpired
    );
    assert_eq!(mock.count(), 1);
    mock.stop().await;
}
