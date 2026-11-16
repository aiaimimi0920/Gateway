//! Real loopback responses cover decoded metadata limits without browser/platform calls.
use super::*;
use axum::{body::Body, response::Response, routing::get, Router};

async fn decode(bytes: Vec<u8>, gzip: bool, provider: &str) -> Result<Option<Value>, GatewayError> {
    let app = Router::new().route(
        "/",
        get(move || {
            let bytes = bytes.clone();
            async move {
                let chunks: Vec<_> = bytes
                    .chunks(1024)
                    .map(|b| Ok::<_, std::io::Error>(bytes::Bytes::copy_from_slice(b)))
                    .collect();
                let mut response = Response::builder().header("content-type", "application/json");
                if gzip {
                    response = response.header("content-encoding", "gzip");
                }
                response
                    .body(Body::from_stream(futures::stream::iter(chunks)))
                    .unwrap()
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let response = rquest::Client::builder()
        .no_proxy()
        .build()
        .unwrap()
        .get(url)
        .send()
        .await
        .unwrap();
    assert_eq!(
        response.content_length(),
        None,
        "decoded/chunked input has no trusted length"
    );
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(3),
        decode_remote_browser_executor_response(
            response,
            RequestTimeBrowserPolicy::RemoteOnly,
            provider,
            "fixture",
            "music_generations",
        ),
    )
    .await
    .unwrap();
    server.abort();
    let _ = server.await;
    result
}
fn metadata(size: usize) -> Vec<u8> {
    let mut value = serde_json::json!({"ok":true,"result":{"pad":""}});
    let base = serde_json::to_vec(&value).unwrap().len();
    value["result"]["pad"] = Value::String("x".repeat(size - base));
    let bytes = serde_json::to_vec(&value).unwrap();
    assert_eq!(bytes.len(), size);
    bytes
}
fn safe_read_failure(result: Result<Option<Value>, GatewayError>) {
    let error = result.unwrap_err();
    assert_eq!(
        error.code.as_deref(),
        Some("browser_executor_required_failed")
    );
    assert_eq!(
        error.message,
        "Remote browser executor response could not be read safely."
    );
}
#[tokio::test]
async fn udio_executor_chunked_metadata_is_bounded_at_128_kib_not_64_mib() {
    assert!(decode(metadata(128 * 1024), false, "udio")
        .await
        .unwrap()
        .is_some());
    safe_read_failure(decode(metadata(128 * 1024 + 1), false, "udio").await);
    assert!(decode(metadata(128 * 1024 + 1), false, "producer")
        .await
        .unwrap()
        .is_some());
}
#[tokio::test]
async fn udio_executor_gzip_budget_measures_decoded_not_wire_bytes() {
    // Deterministic gzip of the valid 131073-byte metadata() envelope, not an audio asset.
    let bytes = hex::decode("1f8b080000000000020aedc1c10980300004b05deeed04dd46b02f05a556104a777710938c9c7b4a6f4f5dd2eafd1c3d65e45ab794bc000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000c08f64ce0fa8ec4aa201000200").unwrap();
    assert!(bytes.len() < 256);
    assert!(decode(bytes.clone(), true, "producer")
        .await
        .unwrap()
        .is_some());
    safe_read_failure(decode(bytes, true, "udio").await);
}
#[tokio::test]
async fn udio_executor_invalid_utf8_and_html_do_not_expose_body() {
    safe_read_failure(decode(vec![0xff], false, "udio").await);
    let error = decode(b"<html>private-result</html>".to_vec(), false, "udio")
        .await
        .unwrap_err();
    assert_eq!(
        error.message,
        "Remote browser executor returned invalid JSON."
    );
    assert!(!error.message.contains("private-result"));
}
