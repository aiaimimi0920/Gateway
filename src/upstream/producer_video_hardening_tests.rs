use super::*;
use axum::response::IntoResponse;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tokio::time::timeout;

struct Server(tokio::task::JoinHandle<()>);

impl Drop for Server {
    fn drop(&mut self) {
        self.0.abort();
    }
}

async fn poll_fixture(body: &'static str, stall: bool) -> Result<Value, GatewayError> {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base_url = format!("http://{}", listener.local_addr().unwrap());
    let (sent, arrived) = oneshot::channel();
    let _server = Server(tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = [0u8; 8192];
        assert!(socket.read(&mut request).await.unwrap() > 0);
        let length = body.len() + if stall { 100 } else { 0 };
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {length}\r\nConnection: close\r\n\r\n{body}"
        );
        socket.write_all(response.as_bytes()).await.unwrap();
        sent.send(()).unwrap();
        if stall {
            std::future::pending::<()>().await;
        }
    }));
    let client = Client::builder().no_proxy().build().unwrap();
    let result = timeout(
        Duration::from_secs(2),
        poll_producer_video_status_until_complete(
            &client,
            &base_url,
            &HeaderMap::new(),
            "video-job-1",
            &base_url,
            Duration::from_millis(200),
        ),
    )
    .await
    .expect("video poll budget must include response bodies and interval sleeps");
    arrived.await.unwrap();
    result
}

#[tokio::test]
async fn video_poll_budget_includes_interval_sleep() {
    let error = poll_fixture(r#"{"status":"processing"}"#, false)
        .await
        .unwrap_err();
    assert_eq!(error.code.as_deref(), Some("producer_video_poll_timeout"));
}

#[tokio::test]
async fn video_poll_budget_includes_stalled_body() {
    let error = poll_fixture("{", true).await.unwrap_err();
    assert_eq!(error.code.as_deref(), Some("producer_video_poll_timeout"));
}

#[tokio::test]
async fn video_poll_preserves_completed_payload() {
    let body = r#"{"status":"completed","items":["https://cdn.example.com/music-video/video-job-1/final.mp4"]}"#;
    assert_eq!(
        poll_fixture(body, false).await.unwrap(),
        serde_json::from_str::<Value>(body).unwrap()
    );
}

#[test]
fn pending_video_response_never_claims_media_completion() {
    for status in ["accepted", "processing", "completed"] {
        let body = build_producer_video_http_response(
            "producer_compatible",
            "https://www.producer.ai",
            &json!({"async": true}),
            "model",
            "clip-1",
            "conv-1",
            "bootstrap-1",
            "creative-1",
            None,
            "video-job-1",
            &json!({"status": status}),
            &json!({}),
            None,
        )
        .unwrap();
        assert_eq!(body["completed"], false);
        assert_eq!(body["accepted"], true);
        assert_eq!(body["state"], status);
        assert!(body["data"][0]["url"].is_null());
    }
}

#[tokio::test]
async fn terminal_video_error_redacts_credentials_in_wire_response() {
    let payload = json!({
        "status": "failed", "token": "fixture-private-token",
        "authorization": "Bearer fixture-private-bearer"
    });
    let error =
        ensure_successful_producer_video_final_status("producer_compatible", "failed", &payload)
            .unwrap_err();
    assert_eq!(error.code.as_deref(), Some("producer_http_video_failed"));
    assert_eq!(error.provider_name.as_deref(), Some("producer_compatible"));
    let response = error.into_response();
    assert_eq!(
        response.status(),
        axum::http::StatusCode::INTERNAL_SERVER_ERROR
    );
    let bytes = axum::body::to_bytes(response.into_body(), 4096)
        .await
        .unwrap();
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    let message = body["error"]["message"].as_str().unwrap();
    assert!(!message.contains("fixture-private-token"));
    assert!(!message.contains("fixture-private-bearer"));
    assert!(message.contains("[REDACTED]"));
}

#[test]
fn terminal_video_error_message_is_bounded() {
    let error =
        producer_http_video_failed_error("producer_compatible", "failed", &"x".repeat(4096));
    assert!(error.message.chars().count() <= crate::error::PROVIDER_ERROR_MESSAGE_MAX_CHARS);
}
