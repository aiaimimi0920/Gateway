//! Multipart ingress, real local routing/auth and selected-worker transport.
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use neuro_gateway::{config::GatewayStorageMode, http::router::build_router};
use serde_json::{json, Value};
use tower::ServiceExt;

fn multipart(audio: &[u8], format: &str, duplicate: bool) -> Vec<u8> {
    let mut bytes = format!("--fixture\r\nContent-Disposition: form-data; name=\"model\"\r\n\r\ngemini-3-flash-preview\r\n--fixture\r\nContent-Disposition: form-data; name=\"response_format\"\r\n\r\n{format}\r\n").into_bytes();
    for _ in 0..if duplicate { 2 } else { 1 } {
        bytes.extend_from_slice(b"--fixture\r\nContent-Disposition: form-data; name=\"file\"; filename=\"tone.mp3\"\r\nContent-Type: audio/mpeg\r\n\r\n");
        bytes.extend_from_slice(audio);
        bytes.extend_from_slice(b"\r\n");
    }
    bytes.extend_from_slice(b"--fixture--\r\n");
    bytes
}

pub(super) async fn exercise(audio: &[u8]) {
    let root = std::env::temp_dir().join(format!("gateway-transcription-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let mut config = super::support::test_config();
    config.storage_mode = GatewayStorageMode::Local;
    config.console.state_dir = root.join("state");
    config.console.routes_file = root.join("routes.yaml");
    std::fs::write(
        &config.console.routes_file,
        r#"
providers:
  - id: fixture-provider
    preset: gemini-canvas-browser-relay
    base_url: https://gemini.google.com
    supported_models: [gemini-3-flash-preview]
    credentials:
      - id: fixture-credential
        api_key: ""
        runtime_state_object_key: fixture/selected-session.json
model_routes:
  - pattern: gemini-3-flash-preview
    provider_ids: [fixture-provider]
"#,
    )
    .unwrap();
    let state = Box::pin(neuro_gateway::runtime::build_app_state(config))
        .await
        .unwrap();
    for (format, auth, duplicate, expected) in [
        ("json", true, false, StatusCode::OK),
        ("text", true, false, StatusCode::OK),
        ("srt", true, false, StatusCode::BAD_REQUEST),
        ("json", false, false, StatusCode::UNAUTHORIZED),
        ("json", true, true, StatusCode::BAD_REQUEST),
    ] {
        let mut request = Request::post("/v1/audio/transcriptions")
            .header("content-type", "multipart/form-data; boundary=fixture");
        if auth {
            request = request.header("authorization", "Bearer public-api-contract-secret");
        }
        let request = request
            .body(Body::from(multipart(audio, format, duplicate)))
            .unwrap();
        let reply = Box::pin(build_router(state.clone()).oneshot(request))
            .await
            .unwrap();
        let status = reply.status();
        let content_type = reply
            .headers()
            .get("content-type")
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();
        let body = reply.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(
            status,
            expected,
            "format={format}, auth={auth}, duplicate={duplicate}: {}",
            String::from_utf8_lossy(&body)
        );
        if status == StatusCode::OK {
            if format == "json" {
                assert_eq!(
                    serde_json::from_slice::<Value>(&body).unwrap(),
                    json!({"text":"合成回执"})
                );
                assert!(content_type.starts_with("application/json"));
            } else {
                assert_eq!(body.as_ref(), "合成回执".as_bytes());
                assert_eq!(content_type, "text/plain; charset=utf-8");
            }
        }
    }
    state.local_runtime.as_ref().unwrap().close().await;
    // Keep isolated synthetic artifacts for receipt inspection; no live data is read.
}
