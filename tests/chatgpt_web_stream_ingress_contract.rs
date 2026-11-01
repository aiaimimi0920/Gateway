//! Real routing/finalizers with synthetic loopback Web SSE, never external accounts.
use axum::{
    body::Body,
    extract::{OriginalUri, State},
    http::{Request, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use http_body_util::BodyExt;
use neuro_gateway::{config::GatewayStorageMode, db, http::router::build_router};
use serde_json::{json, Value};
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};
use tower::ServiceExt;
#[path = "console_contract_support/mod.rs"]
mod support;

#[derive(Clone)]
struct Fixture {
    calls: Arc<AtomicUsize>,
    complete: bool,
}

async fn homepage(State(fixture): State<Fixture>) -> &'static str {
    fixture.calls.fetch_add(1, Ordering::SeqCst);
    "<html></html>"
}

async fn upstream(
    State(fixture): State<Fixture>,
    OriginalUri(uri): OriginalUri,
    Json(body): Json<Value>,
) -> axum::response::Response {
    fixture.calls.fetch_add(1, Ordering::SeqCst);
    if uri.path().ends_with("/prepare") {
        return Json(json!({"prepare_token":"synthetic-prepare"})).into_response();
    }
    if uri.path().ends_with("/finalize") {
        assert_eq!(body["prepare_token"], "synthetic-prepare");
        return Json(json!({"token":"synthetic-requirements"})).into_response();
    }
    assert_eq!(body["model"], "gpt-5.6-sol");
    let end = if fixture.complete {
        "data: [DONE]\n\n"
    } else {
        "data: {\"error\":{\"message\":\"private-upstream-marker\"}}\n\ndata: [DONE]\n\n"
    };
    ([("content-type", "text/event-stream")], format!("data: {{\"message\":{{\"author\":{{\"role\":\"assistant\"}},\"content\":{{\"parts\":[\"answer\"]}}}}}}\n\n{end}")).into_response()
}

async fn exercise(endpoint: &str, complete: bool) {
    let fixture = Fixture {
        calls: Arc::new(AtomicUsize::new(0)),
        complete,
    };
    let app = Router::new()
        .route("/", get(homepage))
        .route(
            "/backend-api/sentinel/chat-requirements/prepare",
            post(upstream),
        )
        .route(
            "/backend-api/sentinel/chat-requirements/finalize",
            post(upstream),
        )
        .route("/backend-api/conversation", post(upstream))
        .with_state(fixture.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let root = std::env::temp_dir().join(format!("chatgpt-web-stream-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let provider = format!("web-stream-{}", uuid::Uuid::new_v4());
    let mut config = support::test_config();
    config.storage_mode = GatewayStorageMode::Local;
    config.console.state_dir = root.join("state");
    config.console.routes_file = root.join("routes.yaml");
    std::fs::write(
        &config.console.routes_file,
        format!(
            r#"
providers:
  - id: {provider}
    preset: chatgpt-web-reverse
    base_url: http://{address}
    supported_models: [gpt-5.6-sol]
    extra_body:
      requestTimeBrowserAllowed: false
    credentials:
      - id: exact-web-credential
        api_key: synthetic-session-secret
        session_auth:
          transport: bearer
          expiresAt: '2099-01-01T00:00:00Z'
model_routes:
  - pattern: gpt-5.6-sol
    provider_ids: [{provider}]
"#
        ),
    )
    .unwrap();
    let state = Box::pin(neuro_gateway::runtime::build_app_state(config))
        .await
        .unwrap();
    let mut body = json!({"model":"gpt-5.6-sol","stream":true,"max_tokens":32});
    match endpoint {
        "responses" => body["input"] = json!("Reply with only OK."),
        "completions" => body["prompt"] = json!("Reply with only OK."),
        _ => body["messages"] = json!([{"role":"user","content":"Reply with only OK."}]),
    }
    let response = Box::pin(
        build_router(state.clone()).oneshot(
            Request::post(format!("/v1/{endpoint}"))
                .header("authorization", "Bearer public-api-contract-secret")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        ),
    )
    .await
    .unwrap();
    if response.status() != StatusCode::OK {
        let status = response.status();
        let body = axum::body::to_bytes(response.into_body(), 1024 * 1024)
            .await
            .unwrap();
        panic!("{endpoint}: {status}: {}", String::from_utf8_lossy(&body));
    }
    let mut body = response.into_body();
    let mut text = String::new();
    let mut failures = 0;
    while let Some(frame) = body.frame().await {
        match frame {
            Ok(frame) => {
                if let Some(bytes) = frame.data_ref() {
                    text.push_str(std::str::from_utf8(bytes).unwrap());
                }
            }
            Err(_) => failures += 1,
        }
    }
    assert!(text.contains("answer"), "{endpoint}: {text}");
    assert_eq!(failures, usize::from(!complete), "{endpoint}: {text}");
    if !complete {
        for terminal in [
            "data: [DONE]",
            "\"finish_reason\":\"stop\"",
            "event: response.completed",
            "event: message_stop",
        ] {
            assert!(!text.contains(terminal), "{endpoint}: {text}");
        }
    }
    assert!(!text.contains("private-upstream-marker"));
    let local = state.local_runtime.as_ref().unwrap();
    let mut audits = Vec::new();
    for _ in 0..100 {
        audits = local
            .list_audits(&db::RequestAuditFilters::default())
            .await
            .unwrap();
        if audits.len() == 1 && matches!(audits[0].status.as_str(), "completed" | "failed") {
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    assert_eq!(audits.len(), 1);
    assert_eq!(
        audits[0].status,
        if complete { "completed" } else { "failed" },
        "{audits:?}"
    );
    assert!(!serde_json::to_string(&audits)
        .unwrap()
        .contains("private-upstream-marker"));
    let observations = serde_json::to_value(
        neuro_gateway::metrics::request::global_gateway_metrics().model_observations(),
    )
    .unwrap();
    let observation = observations
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["providerId"] == provider)
        .expect("preserve the synthetic-model observation");
    assert_eq!(observation["observationStatus"], "synthetic_or_unobserved");
    assert!(observation["declaredModel"].is_null());
    assert_eq!(observation["verified"], false);
    assert_eq!(
        fixture.calls.load(Ordering::SeqCst),
        4,
        "no hidden retry or browser fallback"
    );
    local.close().await;
    server.abort();
    let _ = server.await;
}

#[test]
fn web_stream_failures_reach_protocol_bridges_and_failure_audit() {
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(|| {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async {
                    for endpoint in ["chat/completions", "responses", "messages", "completions"] {
                        exercise(endpoint, false).await;
                    }
                    exercise("chat/completions", true).await;
                });
        })
        .unwrap()
        .join()
        .unwrap();
}
