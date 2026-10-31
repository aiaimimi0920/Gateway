//! Public auth, model policy, routing and binary emission with a synthetic local account.
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use neuro_gateway::{config::GatewayStorageMode, http::router::build_router};
use serde_json::json;
use tower::ServiceExt;

async fn exercise() {
    let upstream = super::fixture(super::frame("XqA3Ic", &super::wav())).await;
    let root = std::env::temp_dir().join(format!("gateway-web-speech-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let mut config = super::support::test_config();
    config.storage_mode = GatewayStorageMode::Local;
    config.console.state_dir = root.join("state");
    config.console.routes_file = root.join("routes.yaml");
    std::fs::write(
        &config.console.routes_file,
        format!(
            r#"
providers:
  - id: synthetic-web-speech
    preset: gemini-web-chat
    base_url: {}
    supported_models: [gemini-web-chat]
    extra_body:
      pureHttpMode: required
      language: en-US
    call_flow:
      special_adapter: gemini_web
      fingerprint_adapter: session
    credentials:
      - id: synthetic-selected
        api_key: synthetic-cookie
model_routes:
  - pattern: gemini-web-chat
    provider_ids: [synthetic-web-speech]
"#,
            upstream.base
        ),
    )
    .unwrap();
    let state = Box::pin(neuro_gateway::runtime::build_app_state(config))
        .await
        .unwrap();
    let mut outcomes = Vec::new();
    for authenticated in [false, true] {
        let mut request =
            Request::post("/v1/audio/speech").header("content-type", "application/json");
        if authenticated {
            request = request.header("authorization", "Bearer public-api-contract-secret");
        }
        let response = Box::pin(build_router(state.clone()).oneshot(request.body(Body::from(json!({
            "model":"gemini-web-chat", "input":"literal synthetic input", "response_format":"wav"
        }).to_string())).unwrap())).await.unwrap();
        let status = response.status();
        let mime = response
            .headers()
            .get("content-type")
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        outcomes.push((authenticated, status, mime, bytes));
    }
    state.local_runtime.as_ref().unwrap().close().await;
    for (authenticated, status, mime, bytes) in outcomes {
        assert_eq!(
            status,
            if authenticated {
                StatusCode::OK
            } else {
                StatusCode::UNAUTHORIZED
            },
            "{}",
            String::from_utf8_lossy(&bytes)
        );
        if authenticated {
            assert_eq!(mime, "audio/wav");
            assert_eq!(bytes.as_ref(), super::wav());
        }
    }
    let captured = upstream.capture.lock().unwrap();
    assert_eq!(captured.gets, 1);
    assert_eq!(captured.calls.len(), 1);
    assert_eq!(
        super::inner(&captured.calls[0].1)[1],
        "literal synthetic input"
    );
}

#[test]
fn public_default_web_speech_keeps_auth_and_returns_exact_binary() {
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(|| {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(exercise());
        })
        .unwrap()
        .join()
        .unwrap();
}
