//! Public multipart/auth/selector/dispatch negatives stay on synthetic local state.
use axum::{
    body::Body,
    http::{Request, StatusCode},
    routing::any,
    Router,
};
use http_body_util::BodyExt;
use neuro_gateway::{config::GatewayStorageMode, http::router::build_router};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use tower::ServiceExt;

#[path = "console_contract_support/mod.rs"]
mod support;

fn multipart(format: &str, audio: &[u8]) -> Vec<u8> {
    let mut bytes = format!("--fixture\r\nContent-Disposition: form-data; name=\"model\"\r\n\r\ngemini-3-flash-preview\r\n--fixture\r\nContent-Disposition: form-data; name=\"response_format\"\r\n\r\n{format}\r\n--fixture\r\nContent-Disposition: form-data; name=\"file\"\r\nContent-Type: audio/mpeg\r\n\r\n").into_bytes();
    bytes.extend_from_slice(audio);
    bytes.extend_from_slice(b"\r\n--fixture--\r\n");
    bytes
}

async fn exercise(preset: &str) {
    let calls = Arc::new(AtomicUsize::new(0));
    let seen = calls.clone();
    let router = Router::new().fallback(any(move || {
        let seen = seen.clone();
        async move {
            seen.fetch_add(1, Ordering::SeqCst);
            StatusCode::BAD_GATEWAY
        }
    }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    let root = std::env::temp_dir().join(format!("gateway-web-ingress-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let mut config = support::test_config();
    config.storage_mode = GatewayStorageMode::Local;
    config.console.state_dir = root.join("state");
    config.console.routes_file = root.join("routes.yaml");
    std::fs::write(
        &config.console.routes_file,
        format!(
            r#"
providers:
  - id: fixture-web
    preset: {preset}
    base_url: http://{address}
    supported_models: [gemini-3-flash-preview]
    credentials:
      - id: fixture-web-credential
        api_key: synthetic-cookie
model_routes:
  - pattern: gemini-3-flash-preview
    provider_ids: [fixture-web]
"#
        ),
    )
    .unwrap();
    let state = Box::pin(neuro_gateway::runtime::build_app_state(config))
        .await
        .unwrap();
    for (format, auth, audio, expected_code, status) in [
        (
            "json",
            true,
            b"".as_slice(),
            "invalid_gemini_web_media_input",
            StatusCode::BAD_REQUEST,
        ),
        (
            "text",
            true,
            b"".as_slice(),
            "invalid_gemini_web_media_input",
            StatusCode::BAD_REQUEST,
        ),
        (
            "srt",
            true,
            b"synthetic audio".as_slice(),
            "invalid_gemini_web_transcription_input",
            StatusCode::BAD_REQUEST,
        ),
        (
            "json",
            false,
            b"synthetic audio".as_slice(),
            "",
            StatusCode::UNAUTHORIZED,
        ),
    ] {
        let mut request = Request::post("/v1/audio/transcriptions")
            .header("content-type", "multipart/form-data; boundary=fixture");
        if auth {
            request = request.header("authorization", "Bearer public-api-contract-secret");
        }
        let response = Box::pin(
            build_router(state.clone())
                .oneshot(request.body(Body::from(multipart(format, audio))).unwrap()),
        )
        .await
        .unwrap();
        let actual = response.status();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let text = String::from_utf8_lossy(&bytes);
        assert_eq!(actual, status, "{format}: {text}");
        if !expected_code.is_empty() {
            assert!(text.contains(expected_code), "{format}: {text}");
        }
    }
    state.local_runtime.as_ref().unwrap().close().await;
    server.abort();
    let _ = server.await;
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    // Retain only this batch's synthetic artifacts for diagnostics, without touching live data.
}

#[test]
fn public_web_transcription_reaches_its_owner_and_rejects_before_network() {
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(|| {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async {
                    for preset in ["gemini-web-chat", "gemini-web-chat-modular"] {
                        exercise(preset).await;
                    }
                });
        })
        .unwrap()
        .join()
        .unwrap();
}
