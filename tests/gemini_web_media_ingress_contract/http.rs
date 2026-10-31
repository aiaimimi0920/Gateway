//! Full public routing with synthetic local accounts; rejected policies must send zero requests.
use axum::{
    body::Body,
    http::{Request, StatusCode},
    routing::any,
    Router,
};
use http_body_util::BodyExt;
use neuro_gateway::{config::GatewayStorageMode, http::router::build_router};
use serde_json::json;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use tower::ServiceExt;

async fn exercise(mode: &str) {
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = calls.clone();
    let app = Router::new().fallback(any(move || {
        let observed = observed.clone();
        async move {
            observed.fetch_add(1, Ordering::SeqCst);
            StatusCode::BAD_GATEWAY
        }
    }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let root = std::env::temp_dir().join(format!("gateway-web-media-{}", uuid::Uuid::new_v4()));
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
  - id: fixture-web
    preset: gemini-web-chat
    base_url: http://{address}
    supported_models: [gemini-2.5-flash-image]
    extra_body:
      pureHttpMode: {mode}
    call_flow:
      special_adapter: gemini_web
      fingerprint_adapter: session
    credentials:
      - id: fixture-web-credential
        api_key: synthetic-cookie
        runtime_state_object_key: synthetic-not-read
model_routes:
  - pattern: gemini-2.5-flash-image
    provider_ids: [fixture-web]
"#
        ),
    )
    .unwrap();
    let state = Box::pin(neuro_gateway::runtime::build_app_state(config))
        .await
        .unwrap();
    let cases = if mode == "required" {
        vec![
            (
                "/v1/images/generations",
                json!({"n":2}),
                "unsupported_gemini_canvas_image_count",
            ),
            (
                "/v1/music/generations",
                json!({}),
                "gemini_web_media_execution_policy_unsupported",
            ),
            (
                "/v1/videos/generations",
                json!({}),
                "gemini_web_media_execution_policy_unsupported",
            ),
        ]
    } else {
        vec![(
            "/v1/images/edits",
            json!({"images":[{"mime_type":"image/png","base64":"aGVsbG8="}]}),
            "gemini_web_media_execution_policy_unsupported",
        )]
    };
    let mut outcomes = vec![];
    for (path, mut body, code) in cases {
        body["model"] = json!("gemini-2.5-flash-image");
        body["prompt"] = json!("synthetic fixture");
        for authenticated in [false, true] {
            let mut request = Request::post(path).header("content-type", "application/json");
            if authenticated {
                request = request.header("authorization", "Bearer public-api-contract-secret");
            }
            let response = Box::pin(
                build_router(state.clone())
                    .oneshot(request.body(Body::from(body.to_string())).unwrap()),
            )
            .await
            .unwrap();
            let status = response.status();
            let bytes = response.into_body().collect().await.unwrap().to_bytes();
            outcomes.push((
                path,
                authenticated,
                status,
                String::from_utf8_lossy(&bytes).into_owned(),
                code,
            ));
        }
    }
    state.local_runtime.as_ref().unwrap().close().await;
    server.abort();
    let _ = server.await;
    assert_eq!(
        calls.load(Ordering::SeqCst),
        0,
        "no generation or bootstrap is allowed"
    );
    for (path, authenticated, status, text, code) in outcomes {
        assert_eq!(
            status,
            if authenticated {
                StatusCode::BAD_REQUEST
            } else {
                StatusCode::UNAUTHORIZED
            },
            "{path}: {text}"
        );
        if authenticated {
            assert!(text.contains(code), "{path}: {text}");
        }
    }
    // Keep only this run's synthetic state under the caller-selected temporary root.
}

#[test]
fn public_default_web_media_routes_keep_auth_and_execution_policy() {
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(|| {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async {
                    exercise("required").await;
                    exercise("disabled").await;
                });
        })
        .unwrap()
        .join()
        .unwrap();
}
