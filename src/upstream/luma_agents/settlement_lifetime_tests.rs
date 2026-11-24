//! Optional settlement shares request cancellation, deadlines and metadata limits.
use super::{settlement, settlement_tests::*, test_support::*, transport};
use serde_json::json;
use std::time::Duration;
use tokio::time::Instant;

#[tokio::test]
async fn luma_agents_settlement_timeout_keeps_media_and_closes_pending_body() {
    let server = Server::start(vec![
        Reply::json(201, pending()),
        Reply::png(),
        Reply {
            hold: true,
            ..Reply::json(200, pending())
        },
    ])
    .await;
    let start = Instant::now();
    let output = run(&server, Duration::from_secs(1)).await;
    let elapsed = start.elapsed();
    assert!(elapsed >= Duration::from_millis(700), "{elapsed:?}");
    assert!(elapsed < Duration::from_millis(950), "{elapsed:?}");
    verified_unknown(&output);
    one_generation(&server, 3);
    server.wait_closed().await;
}

#[tokio::test]
async fn luma_agents_settlement_caps_optional_wait_even_with_a_long_request_deadline() {
    let server = Server::start(vec![
        Reply::json(201, pending()),
        Reply::png(),
        Reply {
            hold: true,
            ..Reply::json(200, pending())
        },
    ])
    .await;
    let start = Instant::now();
    let output = tokio::time::timeout(
        Duration::from_secs(10),
        run(&server, Duration::from_secs(60)),
    )
    .await
    .expect("optional accounting must not consume the whole 60 second request");
    let elapsed = start.elapsed();
    assert!(elapsed >= Duration::from_millis(7900), "{elapsed:?}");
    assert!(elapsed < Duration::from_secs(9), "{elapsed:?}");
    verified_unknown(&output);
    one_generation(&server, 3);
    server.wait_closed().await;
}

#[tokio::test]
async fn luma_agents_settlement_never_starts_before_pending_usage_media_is_verified() {
    let mut invalid = Reply::png();
    invalid.bytes.truncate(40);
    let server = Server::start(vec![Reply::json(201, pending()), invalid]).await;
    let error = within(
        &server.origin,
        super::execute(
            &server.client(),
            Duration::from_secs(5),
            &payload(),
            &request(json!({"prompt":"image"})),
            "uni-1",
        ),
    )
    .await
    .unwrap_err();
    assert_eq!(error.code.as_deref(), Some("luma_agents_asset_type"));
    one_generation(&server, 2);
}

#[tokio::test]
async fn luma_agents_settlement_caller_drop_closes_pending_body_without_background_work() {
    let server = Server::start(vec![
        Reply::json(201, pending()),
        Reply::png(),
        Reply {
            hold: true,
            ..Reply::json(200, pending())
        },
    ])
    .await;
    let origin = server.origin.clone();
    let client = server.client();
    let task = tokio::spawn(async move {
        within(
            &origin,
            super::execute(
                &client,
                Duration::from_secs(5),
                &payload(),
                &request(json!({"prompt":"image"})),
                "uni-1",
            ),
        )
        .await
    });
    tokio::time::timeout(Duration::from_secs(2), async {
        while server.calls().len() < 3 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    server.wait_closed().await;
    one_generation(&server, 3);
}

#[tokio::test]
async fn luma_agents_settlement_reserves_return_time_without_starting_a_read() {
    let server = Server::start(vec![]).await;
    let material = transport::Material::new(&payload()).unwrap();
    let req = crate::protocol::luma_agents::GenerationRequest::pack(
        &request(json!({"prompt":"image"})),
        "uni-1",
    )
    .unwrap();
    for remaining in [Duration::ZERO, Duration::from_millis(200)] {
        let usage = within(
            &server.origin,
            settlement::collect(
                &server.client(),
                &material,
                &req,
                ID,
                &pending(),
                Instant::now() + remaining,
                &mut 0,
            ),
        )
        .await
        .unwrap();
        assert!(usage["credits_charged"].is_null());
        assert!(server.calls().is_empty());
    }
}

#[tokio::test]
async fn luma_agents_settlement_shares_the_existing_metadata_byte_budget() {
    let mut completed = pending();
    completed["usage"]["credits_charged"] = json!(42);
    let server = Server::start(vec![Reply::json(200, completed)]).await;
    let material = transport::Material::new(&payload()).unwrap();
    let req = crate::protocol::luma_agents::GenerationRequest::pack(
        &request(json!({"prompt":"image"})),
        "uni-1",
    )
    .unwrap();
    let mut total = 4 * 1024 * 1024;
    let usage = within(
        &server.origin,
        settlement::collect(
            &server.client(),
            &material,
            &req,
            ID,
            &pending(),
            Instant::now() + Duration::from_secs(5),
            &mut total,
        ),
    )
    .await
    .unwrap();
    assert!(usage["credits_charged"].is_null());
    assert_eq!(server.calls().len(), 1);
    assert_eq!(server.calls()[0].method, "GET");
}
