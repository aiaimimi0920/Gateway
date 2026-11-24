//! Transport timeouts must retain their meaning at the HTTP boundary.
use axum::response::IntoResponse;
use neuro_gateway::error::{classify_network_error, ErrorKind};
use std::time::Duration;

#[tokio::test]
async fn upstream_transport_timeout_returns_gateway_timeout() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let client = rquest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_millis(100))
        .build()
        .unwrap();
    let error = client
        .get(format!("http://{address}/timeout"))
        .send()
        .await
        .unwrap_err();
    assert!(error.is_timeout());
    let classified = classify_network_error(&error, Some("timeout-fixture"));
    assert_eq!(classified.kind, ErrorKind::Timeout);
    assert!(classified.retryable);
    assert_eq!(classified.into_response().status(), 504);
}
