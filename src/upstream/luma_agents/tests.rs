use super::{execute, test_support::*};
use crate::error::ErrorKind;
use serde_json::json;
use std::time::Duration;

#[tokio::test]
async fn luma_agents_submit_poll_download_is_one_generation_with_isolated_auth() {
    let server = Server::start(vec![
        Reply::json(201, job("queued")),
        Reply::json(200, job("completed")),
        Reply::png(),
    ])
    .await;
    let result=within(&server.origin,execute(&server.client(),Duration::from_secs(5),&payload(),
        &request(json!({"model":"caller-alias","prompt":"中文图像 &amp; literal","response_format":"b64_json"})),"uni-1")).await.unwrap();
    assert!(!result["data"][0]["b64_json"].as_str().unwrap().is_empty());
    assert_eq!(result["gateway_luma_generation"]["id"], ID);
    assert!(result["gateway_luma_generation"]["usage"]["credits_charged"].is_null());
    let calls = server.calls();
    assert_eq!(calls.len(), 3);
    assert_eq!(
        (calls[0].method.as_str(), calls[0].path.as_str()),
        ("POST", "/v1/generations")
    );
    assert_eq!(calls[0].body["prompt"], "中文图像 &amp; literal");
    assert_eq!(calls[0].body["model"], "uni-1");
    for call in &calls[..2] {
        assert_eq!(
            call.headers["authorization"],
            "Bearer luma-api-synthetic-only"
        );
        assert!(!call.headers.contains_key("cookie"));
    }
    assert_eq!(calls[1].path, format!("/v1/generations/{ID}"));
    assert!(!calls[2].headers.contains_key("authorization"));
    assert!(!calls[2].headers.contains_key("cookie"));
}

#[tokio::test]
async fn luma_agents_failed_generation_never_downloads_or_resubmits() {
    let mut failed = job("failed");
    failed["failure_code"] = json!("content_moderated");
    failed["failure_reason"] = json!("private supplier diagnostic luma-api-secret");
    let server = Server::start(vec![Reply::json(201, failed)]).await;
    let error = within(
        &server.origin,
        execute(
            &server.client(),
            Duration::from_secs(5),
            &payload(),
            &request(json!({"model":"uni-1","prompt":"image"})),
            "uni-1",
        ),
    )
    .await
    .unwrap_err();
    assert_eq!(error.kind, ErrorKind::ContentFilter);
    assert_eq!(error.code.as_deref(), Some("luma_agents_content_moderated"));
    assert!(!error.retryable);
    assert!(!error.message.contains("private"));
    assert_eq!(server.calls().len(), 1);
}

#[tokio::test]
async fn luma_agents_completed_url_with_html_is_not_a_success() {
    let server = Server::start(vec![
        Reply::json(201, job("completed")),
        Reply {
            status: 200,
            content_type: "text/html",
            bytes: b"<html>login</html>".to_vec(),
            hold: false,
        },
    ])
    .await;
    let error = within(
        &server.origin,
        execute(
            &server.client(),
            Duration::from_secs(5),
            &payload(),
            &request(json!({"model":"uni-1","prompt":"image"})),
            "uni-1",
        ),
    )
    .await
    .unwrap_err();
    assert_eq!(error.code.as_deref(), Some("luma_agents_asset_type"));
    assert_eq!(server.calls().len(), 2);
}

#[tokio::test]
async fn luma_agents_total_timeout_does_not_restart_generation() {
    let server = Server::start(vec![Reply {
        hold: true,
        ..Reply::json(201, job("queued"))
    }])
    .await;
    let error = within(
        &server.origin,
        execute(
            &server.client(),
            Duration::from_millis(120),
            &payload(),
            &request(json!({"model":"uni-1","prompt":"image"})),
            "uni-1",
        ),
    )
    .await
    .unwrap_err();
    assert_eq!(error.kind, ErrorKind::Timeout);
    assert_eq!(server.calls().len(), 1);
    server.wait_closed().await;
}
