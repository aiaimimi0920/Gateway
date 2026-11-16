//! Red/green behavior contracts use the real executor, not an alternate transport implementation.
use super::test_support::{self as fixture, Reply, Server, ASSET};
use crate::{protocol::canonical::EndpointKind, upstream::client::UpstreamClient};
use serde_json::json;

#[tokio::test]
async fn chataibot_url_success_requires_a_valid_downloaded_image() {
    let server = Server::start(vec![
        Reply::json(json!({})),
        Reply::json(json!({"imageUrl":ASSET})),
        Reply {
            bytes: b"not a PNG".to_vec(),
            ..Reply::png()
        },
    ])
    .await;
    let req = fixture::request(json!({"prompt":"synthetic","model":"google-nano-banana-2"}));
    let result = fixture::within(
        &server.origin,
        UpstreamClient::new(5).execute_chataibot_images(
            &fixture::payload(&server.origin),
            &req,
            "google-nano-banana-2",
            None,
        ),
    )
    .await;
    assert!(
        result.is_err(),
        "an unvalidated URL cannot count as a successful image: {result:?}"
    );
    assert_eq!(server.calls().len(), 3);
    server.stop().await;
}

#[tokio::test]
async fn chataibot_rejects_unsupported_edit_before_settings_write() {
    let server = Server::start(vec![Reply::json(json!({}))]).await;
    let mut req =
        fixture::request(json!({"prompt":"synthetic","image":"data:image/png;base64,aGVsbG8="}));
    req.endpoint_kind = EndpointKind::ImagesEdits;
    let result = fixture::within(
        &server.origin,
        UpstreamClient::new(5).execute_chataibot_images(
            &fixture::payload(&server.origin),
            &req,
            "ideogram",
            None,
        ),
    )
    .await;
    assert!(result.is_err());
    assert_eq!(
        server.calls().len(),
        0,
        "bad edit must not mutate account settings"
    );
    server.stop().await;
}

#[tokio::test]
async fn chataibot_pending_job_is_polled_without_resubmission() {
    let server = Server::start(vec![
        Reply::json(json!({})),
        Reply::json(json!({"status":"pending","jobId":"synthetic-job-1"})),
        Reply::json(json!({"status":"completed","data":{"imageUrl":ASSET}})),
        Reply::png(),
    ])
    .await;
    let req = fixture::request(json!({"prompt":"synthetic","response_format":"b64_json"}));
    let result = fixture::within(
        &server.origin,
        UpstreamClient::new(5).execute_chataibot_images(
            &fixture::payload(&server.origin),
            &req,
            "google-nano-banana-2",
            None,
        ),
    )
    .await;
    assert!(
        result.is_ok(),
        "pending is not missing-image failure: {result:?}"
    );
    assert_eq!(
        server
            .calls()
            .iter()
            .map(|c| c.path.as_str())
            .collect::<Vec<_>>(),
        [
            "/api/user/update",
            "/api/image/generate",
            "/api/jobs/synthetic-job-1",
            "/image.png?signature=synthetic-private"
        ]
    );
    server.stop().await;
}
