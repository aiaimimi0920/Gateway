//! Full router/auth/route/send/body consumption, with synthetic credentials and random loopback.
#[path = "console_contract_support/mod.rs"]
mod support;
#[path = "bedrock_native_outbound_contract/fixture.rs"]
mod wire;

use axum::{
    body::Body,
    extract::State,
    http::{header, HeaderMap, Request, StatusCode},
    routing::post,
    Json, Router,
};
use bytes::Bytes;
use http_body_util::BodyExt;
use neuro_gateway::http::router::build_router;
use serde_json::{json, Value};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use tower::ServiceExt;

#[derive(Clone)]
struct Upstream {
    calls: Arc<AtomicUsize>,
    reply: &'static str,
}

async fn respond(
    State(upstream): State<Upstream>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> ([(header::HeaderName, &'static str); 1], &'static str) {
    assert_eq!(headers["authorization"], "Bearer synthetic-only");
    assert_eq!(body["model"], "fixture-model");
    assert_eq!(body["stream"], true);
    assert_eq!(body["messages"][0]["content"], "Reply with only OK.");
    upstream.calls.fetch_add(1, Ordering::SeqCst);
    (
        [(header::CONTENT_TYPE, "text/event-stream")],
        upstream.reply,
    )
}

async fn verify_async(reply: &'static str, valid: bool) {
    let calls = Arc::new(AtomicUsize::new(0));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let app = Router::new()
        .route("/v1/chat/completions", post(respond))
        .with_state(Upstream {
            calls: calls.clone(),
            reply,
        });
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let fixture = support::ConsoleStateFixture::new(serde_json::from_value(json!({
        "providers":[{"id":"native-bedrock-output", "adapter":"openai_compatible",
            "protocol_profile":"openai", "base_url":format!("http://{address}"),
            "call_flow":{"call_adapter":"auto","fingerprint_adapter":"auto","special_adapter":"auto"},
            "supported_models":["fixture-model"], "credentials":[{"id":"exact-account",
                "api_key":"synthetic-only", "supported_models":["fixture-model"]}]}],
        "model_routes":[{"pattern":"fixture-model","provider_ids":["native-bedrock-output"]}],
        "aliases":{}})).unwrap(), false);
    let candidates = fixture
        .state
        .route_config
        .snapshot()
        .resolve_candidates(Some("fixture-model"));
    assert_eq!(candidates.len(), 1);
    let controller = fixture
        .state
        .concurrency_registry
        .get_or_create(&candidates[0].provider_account_id);
    let response = build_router(fixture.state.clone())
        .oneshot(
            Request::post("/model/fixture-model/converse-stream")
                .header("authorization", "Bearer public-api-contract-secret")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"messages":[{"role":"user","content":[{
                "text":"Reply with only OK."}]}]})
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    assert_eq!(
        response.headers().get(header::CONTENT_TYPE).unwrap(),
        "application/vnd.amazon.eventstream"
    );
    assert_eq!(controller.snapshot().consecutive_successes, 0);
    let mut body = response.into_body();
    let mut bytes = Vec::new();
    let mut failed = false;
    while let Some(frame) = body.frame().await {
        match frame {
            Ok(frame) => {
                if let Ok(data) = frame.into_data() {
                    bytes.extend_from_slice(&data);
                }
            }
            Err(_) => {
                failed = true;
                break;
            }
        }
        assert!(bytes.len() < 64 * 1024);
    }
    drop(body);
    server.abort();
    let _ = server.await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(failed, !valid, "HTTP 200 alone must not certify the body");
    assert_eq!(
        controller.snapshot().consecutive_successes,
        usize::from(valid)
    );
    assert_eq!(controller.snapshot().active_count, 0);
    let mut events = Vec::new();
    while !bytes.is_empty() {
        let length = u32::from_be_bytes(bytes[..4].try_into().unwrap()) as usize;
        assert!(length <= bytes.len());
        events.push(wire::decode(&Bytes::copy_from_slice(&bytes[..length])));
        bytes.drain(..length);
    }
    assert_eq!(events.first().unwrap().0, "messageStart");
    if valid {
        assert_eq!(events.last().unwrap().0, "metadata");
        assert_eq!(events.iter().filter(|v| v.0 == "messageStop").count(), 1);
        assert_eq!(events.last().unwrap().1["usage"]["totalTokens"], 3);
    } else {
        assert!(events
            .iter()
            .all(|v| !matches!(v.0.as_str(), "messageStop" | "metadata")));
    }
}

fn verify(reply: &'static str, valid: bool) {
    // Windows defaults to a 2 MiB test stack, unlike the real runtime's budget.
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async {
                    tokio::time::timeout(
                        std::time::Duration::from_secs(10),
                        verify_async(reply, valid),
                    )
                    .await
                    .unwrap();
                });
        })
        .unwrap()
        .join()
        .unwrap();
}

const CONTENT: &str = "data: {\"choices\":[{\"delta\":{\"content\":\"OK\"}}]}\n\n";
#[test]
fn native_route_encodes_once_and_consumes_the_valid_body_before_committing_success() {
    verify(concat!("data: {\"choices\":[{\"delta\":{\"content\":\"OK\"}}]}\n\n",
        "data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}],\"usage\":{\"prompt_tokens\":1,\"completion_tokens\":2,\"total_tokens\":3}}\n\n",
        "data: [DONE]\n\n"), true);
}

#[test]
fn native_route_body_failure_after_http_200_never_commits_success() {
    verify(CONTENT, false);
}
