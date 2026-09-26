use super::{execute_driver, CredentialAutomationDriver, CredentialAutomationDriverTransport};
use super::{CredentialPoolAutomationConfig, DriverResponse};
use std::time::Duration;

mod fixture;
use fixture::{request, Reply, Server};

const RESPONSE_LIMIT: usize = 2 * 1024 * 1024;
const LIMIT_ERROR: &str = "credential automation driver response exceeded the size limit";

async fn invoke(reply: Reply) -> anyhow::Result<DriverResponse> {
    let server = Server::start(reply).await;
    let driver = CredentialAutomationDriver {
        id: "loopback-driver".to_string(),
        provider_ids: vec!["provider-a".to_string()],
        timeout_secs: Some(10),
        transport: CredentialAutomationDriverTransport::Http {
            endpoint: server.endpoint.clone(),
            secret_env: None,
        },
    };
    let request = request();
    let result = tokio::time::timeout(
        Duration::from_secs(2),
        execute_driver(
            &CredentialPoolAutomationConfig::default(),
            &driver,
            &request,
        ),
    )
    .await;
    let observed = server.finish().await;
    assert_eq!(observed.request_line, "POST /driver HTTP/1.1");
    assert_eq!(observed.content_type, "application/json");
    assert_eq!(observed.body, serde_json::to_value(request).unwrap());
    result.expect("automation HTTP driver exceeded the response decision budget")
}

fn json_at_length(length: usize) -> Vec<u8> {
    let mut bytes = br#"{"credentials":[],"prune":[],"message":""#.to_vec();
    assert!(length >= bytes.len() + 2);
    bytes.resize(length - 2, b'a');
    bytes.extend_from_slice(b"\"}");
    bytes
}

#[tokio::test]
async fn oversized_declared_length_is_rejected_before_body_arrival() {
    let mut reply = Reply::fixed(Vec::new());
    reply.declared_length = Some(RESPONSE_LIMIT + 1);
    reply.hold_open = true;
    assert_eq!(invoke(reply).await.unwrap_err().to_string(), LIMIT_ERROR);
}

#[tokio::test]
async fn oversized_chunked_body_is_rejected_before_the_stream_ends() {
    let reply = Reply::chunked(vec![b' '; RESPONSE_LIMIT + 1], true);
    assert_eq!(invoke(reply).await.unwrap_err().to_string(), LIMIT_ERROR);
}

#[tokio::test]
async fn exact_limit_json_is_accepted_without_truncation() {
    let body = json_at_length(RESPONSE_LIMIT);
    let expected: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let response = invoke(Reply::fixed(body)).await.unwrap();
    assert!(response.credentials.is_empty());
    assert!(response.prune.is_empty());
    assert_eq!(response.message.as_deref(), expected["message"].as_str());
}

#[tokio::test]
async fn ordinary_chunked_json_preserves_the_driver_response() {
    let body = br#"{"credentials":[{"id":"draft-a"}],"prune":[],"message":"ready"}"#.to_vec();
    let response = invoke(Reply::chunked(body, false)).await.unwrap();
    assert_eq!(response.credentials.len(), 1);
    assert_eq!(response.credentials[0].id.as_deref(), Some("draft-a"));
    assert!(response.prune.is_empty());
    assert_eq!(response.message.as_deref(), Some("ready"));
}

#[tokio::test]
async fn finite_oversized_json_keeps_the_original_size_error() {
    let error = invoke(Reply::fixed(json_at_length(RESPONSE_LIMIT + 1)))
        .await
        .unwrap_err();
    assert_eq!(error.to_string(), LIMIT_ERROR);
}

#[tokio::test]
async fn malformed_json_keeps_the_original_parse_error() {
    let error = invoke(Reply::fixed(b"not-json".to_vec()))
        .await
        .unwrap_err();
    assert_eq!(
        error.to_string(),
        "credential automation driver returned invalid JSON"
    );
}

#[tokio::test]
async fn non_success_status_is_rejected_without_reading_the_body() {
    let mut reply = Reply::fixed(Vec::new());
    reply.status = "503 Service Unavailable";
    reply.declared_length = Some(RESPONSE_LIMIT + 1);
    reply.hold_open = true;
    let error = invoke(reply).await.unwrap_err();
    assert_eq!(
        error.to_string(),
        "credential automation HTTP endpoint returned a non-success status"
    );
}

#[tokio::test]
async fn truncated_body_keeps_the_original_read_error() {
    let mut reply = Reply::fixed(b"{".to_vec());
    reply.declared_length = Some(16);
    let error = invoke(reply).await.unwrap_err();
    assert_eq!(
        error.to_string(),
        "failed to read credential automation HTTP response"
    );
}
