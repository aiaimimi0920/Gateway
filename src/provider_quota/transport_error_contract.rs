mod fixture;

use super::accio_http::fetch_accio_quota_endpoint;
use super::generic_balance::fetch_generic_balance_snapshot;
use super::GatewayProviderQuotaView;
use crate::error::{ErrorKind, GatewayError};
use fixture::{LoopbackFixture, Reply, SECRET};

async fn accio_probe(
    fixture: &LoopbackFixture,
) -> Result<(String, serde_json::Value), GatewayError> {
    fetch_accio_quota_endpoint(
        2,
        &fixture.payload("accio_compatible"),
        "/api/entitlement/currentSubscription",
        "accio_current_subscription",
        false,
    )
    .await
}

async fn balance_probe(
    fixture: &LoopbackFixture,
) -> Result<GatewayProviderQuotaView, GatewayError> {
    fetch_generic_balance_snapshot(
        2,
        "quota-contract-account",
        Some("quota-contract-credential"),
        &fixture.payload("openai_compatible"),
    )
    .await
}

fn assert_private_transport_error(error: &GatewayError, prefix: &str, code: Option<&str>) {
    assert_eq!(error.kind, ErrorKind::ServiceUnavailable);
    assert_eq!(error.http_status, Some(503));
    assert_eq!(error.code.as_deref(), code);
    assert!(error.message.starts_with(prefix));
    assert!(
        !error.message.contains(SECRET),
        "transport diagnostic exposed URL credentials"
    );
    assert!(!error.message.contains("accessToken"));
}

async fn assert_accio_failure(reply: Reply, prefix: &str, code: Option<&str>) {
    let mut fixture = LoopbackFixture::new(reply).await;
    let error = accio_probe(&fixture).await.unwrap_err();
    let request = fixture.finish().await;
    assert!(request.contains(&format!("accessToken={SECRET}")));
    assert_private_transport_error(&error, prefix, code);
}

async fn assert_balance_failure(reply: Reply, prefix: &str, code: Option<&str>) {
    let mut fixture = LoopbackFixture::new(reply).await;
    let error = balance_probe(&fixture).await.unwrap_err();
    let request = fixture.finish().await;
    assert!(request.starts_with(&format!("GET /balance?accessToken={SECRET} ")));
    assert_private_transport_error(&error, prefix, code);
}

#[tokio::test]
async fn accio_transport_failure_keeps_query_credentials_out_of_error() {
    assert_accio_failure(
        Reply::Disconnect,
        "Accio quota request failed:",
        Some("provider_quota_request_failed"),
    )
    .await;
}

#[tokio::test]
async fn balance_transport_failure_keeps_query_credentials_out_of_error() {
    assert_balance_failure(
        Reply::Disconnect,
        "provider balance request failed:",
        Some("provider_quota_request_failed"),
    )
    .await;
}

#[tokio::test]
async fn accio_body_failure_keeps_classification_without_url() {
    assert_accio_failure(Reply::TruncatedBody, "read Accio quota response:", None).await;
}

#[tokio::test]
async fn balance_body_failure_keeps_classification_without_url() {
    assert_balance_failure(
        Reply::TruncatedBody,
        "read provider balance response:",
        None,
    )
    .await;
}

#[tokio::test]
async fn accio_success_retains_control_plane_request() {
    let mut fixture = LoopbackFixture::new(Reply::Http(
        200,
        r#"{"success":true,"data":{"total":100,"remaining":75}}"#,
    ))
    .await;
    let (source, body) = accio_probe(&fixture).await.unwrap();
    let request = fixture.finish().await;
    assert_eq!(source, "accio_current_subscription");
    assert_eq!(body["data"]["remaining"], 75);
    assert!(request.starts_with("GET /api/entitlement/currentSubscription?"));
    assert!(request.contains(&format!("accessToken={SECRET}")));
    assert!(request.contains("utdid=quota-contract-device"));
    assert!(request.contains("version=0.5.6"));
    let headers = request.to_ascii_lowercase();
    assert!(headers.contains("x-utdid: quota-contract-device\r\n"));
    assert!(headers.contains("x-cna: quota-contract-cna\r\n"));
}

#[tokio::test]
async fn balance_success_retains_quota_projection() {
    let mut fixture =
        LoopbackFixture::new(Reply::Http(200, r#"{"remaining":80,"total":100}"#)).await;
    let snapshot = balance_probe(&fixture).await.unwrap();
    let request = fixture.finish().await;
    assert_eq!(snapshot.status, "available");
    assert!(snapshot.ready);
    assert_eq!(snapshot.provider_account_id, "quota-contract-account");
    assert_eq!(
        snapshot.provider_credential_id.as_deref(),
        Some("quota-contract-credential")
    );
    assert_eq!(
        snapshot.raw_data,
        serde_json::json!({ "remaining": 80, "total": 100 })
    );
    assert!(request.contains(&format!("accessToken={SECRET}")));
    assert!(request
        .to_ascii_lowercase()
        .contains(&format!("authorization: bearer {SECRET}\r\n")));
}

async fn assert_http_failure(status: u16, expected_status: u16, kind: ErrorKind) {
    let mut fixture =
        LoopbackFixture::new(Reply::Http(status, "quota temporarily unavailable")).await;
    let error = balance_probe(&fixture).await.unwrap_err();
    fixture.finish().await;
    assert_eq!(error.kind, kind);
    assert_eq!(error.http_status, Some(expected_status));
    assert_eq!(error.code.as_deref(), Some("provider_quota_http_error"));
    assert!(error.message.contains(&format!("status {status}")));
    assert!(error.message.contains("quota temporarily unavailable"));
}

#[tokio::test]
async fn balance_client_http_error_remains_conflict() {
    assert_http_failure(429, 409, ErrorKind::BadRequest).await;
}

#[tokio::test]
async fn balance_server_http_error_remains_unavailable() {
    assert_http_failure(503, 503, ErrorKind::ServiceUnavailable).await;
}
