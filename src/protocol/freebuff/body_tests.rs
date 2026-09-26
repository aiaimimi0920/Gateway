use super::*;
use crate::protocol::freebuff;
use freebuff::tests::{make_payload, make_request};
use serde_json::json;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

struct Fixture {
    payload: crate::routing::candidate::ProviderAccountPayload,
    config: FreeBuffRuntimeConfig,
    client: Client,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.task.abort();
        session_buckets().remove(&self.config.session_bucket_key);
        freebuff::run_buckets().remove(&self.config.bucket_key);
    }
}

fn reply(status: u16, body: &str) -> String {
    format!(
        "HTTP/1.1 {status} Test\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

fn oversized(status: u16) -> String {
    "HTTP/1.1 STATUS Test\r\nContent-Length: 67108865\r\nConnection: close\r\n\r\n"
        .replace("STATUS", &status.to_string())
}

async fn fixture(responses: Vec<String>, paid: bool) -> Fixture {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let mut payload = make_payload();
    payload.base_url = format!("http://{address}");
    payload.credential_id = Some(format!("body-test-{address}"));
    if paid {
        payload
            .extra_body
            .as_mut()
            .unwrap()
            .insert("freebuffCostMode".into(), json!("paid"));
    }
    let config = FreeBuffRuntimeConfig::from_payload(&payload, "z-ai/glm-5.1").unwrap();
    let task = tokio::spawn(async move {
        for response in responses {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = [0u8; 4096];
            let _ = socket.read(&mut request).await.unwrap();
            socket.write_all(response.as_bytes()).await.unwrap();
        }
    });
    Fixture {
        payload,
        config,
        client: Client::builder()
            .timeout(Duration::from_secs(2))
            .build()
            .unwrap(),
        task,
    }
}

fn assert_too_large(error: GatewayError) {
    assert_eq!(error.code.as_deref(), Some("upstream_body_too_large"));
    assert_eq!(error.provider_name.as_deref(), Some("freebuff_compatible"));
}

#[tokio::test]
async fn session_rejects_oversized_declared_body() {
    let f = fixture(vec![oversized(200)], false).await;
    assert_too_large(ensure_free_session(&f.client, &f.config).await.unwrap_err());
}

#[tokio::test]
async fn run_start_and_finish_reject_oversized_declared_bodies() {
    let f = fixture(vec![oversized(200)], true).await;
    assert_too_large(
        freebuff::transport::start_run(&f.client, &f.config)
            .await
            .unwrap_err(),
    );
    let f = fixture(vec![oversized(500)], true).await;
    assert_too_large(
        freebuff::transport::finish_run(&f.client, &f.config, "run", 0)
            .await
            .unwrap_err(),
    );
}

#[tokio::test]
async fn chat_success_and_error_limits_release_the_run_lease() {
    for status in [200, 500] {
        let f = fixture(
            vec![reply(200, r#"{"runId":"run"}"#), oversized(status)],
            true,
        )
        .await;
        let error = freebuff::execute(
            freebuff::run::test_runtime(),
            &f.client,
            &f.payload,
            &make_request(),
            "z-ai/glm-5.1",
            None,
        )
        .await
        .unwrap_err();
        assert_too_large(error);
        let bucket = freebuff::run_bucket(&f.config.bucket_key).unwrap();
        assert_eq!(bucket.lock().await.active.as_ref().unwrap().inflight(), 0);
    }
}

#[tokio::test]
async fn ordinary_chat_and_invalid_json_preserve_result_policy() {
    for (body, valid) in [
        (
            r#"{"id":"chat","model":"model","choices":[{"index":0,"message":{"role":"assistant","content":"hello"},"finish_reason":"stop"}]}"#,
            true,
        ),
        ("{invalid", false),
    ] {
        let f = fixture(
            vec![reply(200, r#"{"runId":"run"}"#), reply(200, body)],
            true,
        )
        .await;
        let result = freebuff::execute(
            freebuff::run::test_runtime(),
            &f.client,
            &f.payload,
            &make_request(),
            "z-ai/glm-5.1",
            None,
        )
        .await;
        if valid {
            assert!(result.is_ok());
        } else {
            let error = result.unwrap_err();
            assert_eq!(error.kind, crate::error::ErrorKind::Unknown);
            assert_eq!(error.code, None);
            assert_eq!(error.http_status, None);
            assert!(!error.retryable);
            assert!(matches!(
                error.fallback_hint,
                crate::error::FallbackHint::FallbackProvider { .. }
            ));
        }
        let bucket = freebuff::run_bucket(&f.config.bucket_key).unwrap();
        assert_eq!(bucket.lock().await.active.as_ref().unwrap().inflight(), 0);
    }
}
