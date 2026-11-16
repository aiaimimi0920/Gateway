use super::{
    fixture::*,
    transport::{exchange, Request},
    *,
};
use serde_json::json;
use std::{sync::Arc, time::Duration};

async fn post(mock: &Mock, budget: Duration) -> Result<Reply, LoginError> {
    TEST_ORIGIN
        .scope(
            mock.origin.clone(),
            exchange(
                &LoginTransport::new().unwrap(),
                Request::Code {
                    fingerprint: "synthetic-device",
                },
                budget,
            ),
        )
        .await
}
#[tokio::test]
async fn redirects_do_not_forward_device_material_or_follow_another_product() {
    let trap = Mock::new(vec![]).await;
    for status in [301, 302, 303, 307, 308] {
        let mut reply = code();
        reply.status = status;
        reply.headers.insert(
            "location",
            format!("{}/api/auth/cli/code", trap.origin)
                .parse()
                .unwrap(),
        );
        let mock = Mock::new(vec![reply]).await;
        assert_eq!(
            post(&mock, Duration::from_secs(2)).await.err(),
            Some(LoginError::RedirectRejected)
        );
        assert_eq!(mock.count(), 1);
        assert_eq!(trap.count(), 0);
        mock.stop().await;
    }
    trap.stop().await;
}

#[tokio::test]
async fn decoded_byte_budget_rejects_chunked_gzip_and_declared_oversize() {
    let mut chunked = code();
    chunked.body = vec![b'x'; 65_537];
    let mut declared = code();
    declared
        .headers
        .insert("content-length", "65537".parse().unwrap());
    declared.body = vec![b'x'; 65_537];
    let mut compressed = code();
    compressed.body = hex::decode("1f8b080000000000020aedc1010100000082a0dbceaf2140010000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000c00d81d82a9c01000100").unwrap();
    compressed
        .headers
        .insert("content-encoding", "gzip".parse().unwrap());
    for reply in [chunked, declared, compressed] {
        let mock = Mock::new(vec![reply]).await;
        assert_eq!(
            post(&mock, Duration::from_secs(2)).await.err(),
            Some(LoginError::ResponseTooLarge)
        );
        assert_eq!(mock.count(), 1);
        mock.stop().await;
    }
}

#[tokio::test]
async fn send_and_read_share_a_deadline_and_cancellation_drops_the_response() {
    for (headers, body) in [(140, 0), (0, 140), (40, 110)] {
        let mut reply = code();
        reply.header_delay = Duration::from_millis(headers);
        reply.body_delay = Duration::from_millis(body);
        let mock = Mock::new(vec![reply]).await;
        assert_eq!(
            post(&mock, Duration::from_millis(80)).await.err(),
            Some(LoginError::Timeout)
        );
        assert_eq!(mock.count(), 1);
        mock.stop().await;
    }
    let mut reply = code();
    reply.body_delay = Duration::from_secs(60);
    let mock = Mock::new(vec![reply]).await;
    let reading = Arc::new(tokio::sync::Notify::new());
    let mut pending =
        Box::pin(TEST_BODY_STARTED.scope(reading.clone(), post(&mock, Duration::from_secs(15))));
    tokio::select! { _=reading.notified()=>{}, result=&mut pending=>panic!("unexpected result: {:?}",result.err()) }
    drop(pending);
    mock.wait_drops(1).await;
    assert_eq!(mock.count(), 1);
    mock.stop().await;
}

#[tokio::test]
async fn cancelled_status_step_is_paced_and_cannot_start_another_post_or_poll_after_expiry() {
    let mut reply = Spec::json(200, json!({}));
    reply.body_delay = Duration::from_secs(60);
    let mock = Mock::new(vec![code(), reply]).await;
    let http = LoginTransport::new().unwrap();
    let mut challenge = mock.begin(&http).await;
    let reading = Arc::new(tokio::sync::Notify::new());
    let mut pending = Box::pin(TEST_BODY_STARTED.scope(
        reading.clone(),
        TEST_ORIGIN.scope(mock.origin.clone(), challenge.poll(&http)),
    ));
    tokio::select! { _=reading.notified()=>{}, result=&mut pending=>panic!("unexpected result: {result:?}") }
    drop(pending);
    mock.wait_drops(2).await;
    assert!(
        challenge
            .next_poll
            .saturating_duration_since(tokio::time::Instant::now())
            > Duration::from_secs(4)
    );
    challenge.deadline = tokio::time::Instant::now() + Duration::from_millis(30);
    assert_eq!(
        TEST_ORIGIN
            .scope(mock.origin.clone(), challenge.poll(&http))
            .await
            .unwrap_err(),
        LoginError::Expired
    );
    assert_eq!(mock.count(), 2);
    mock.stop().await;
}
