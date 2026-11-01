//! Public music ingress with synthetic loopback HTTP only; never opens a browser or spends credits.
use axum::{
    body::Body,
    extract::State,
    http::{HeaderMap, Request, StatusCode},
    routing::{get, post},
    Json, Router,
};
use neuro_gateway::{http::router::build_router, local_runtime::LocalRuntime};
use serde_json::{json, Value};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use tower::ServiceExt;
#[path = "console_contract_support/mod.rs"]
mod support;

async fn conversation(
    State(calls): State<Arc<AtomicUsize>>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Json<Value> {
    assert_eq!(
        headers["authorization"],
        "Bearer synthetic-flowmusic-session"
    );
    assert_eq!(body["client_context"]["selected_model"], "Lyria 3.5");
    calls.fetch_add(1, Ordering::SeqCst);
    Json(json!({"job_id":"synthetic-job"}))
}

async fn exercise(fail_stream: bool) {
    let calls = Arc::new(AtomicUsize::new(0));
    let app = Router::new()
        .route("/__api/conversation", post(conversation))
        .route("/__api/messages/synthetic-job/stream", get(move || async move {
            let body = if fail_stream { "event: error\ndata: {\"message\":\"Synthetic stream failure after acceptance\"}\n\n" }
                else { "event: part\ndata: {\"clip_id\":\"synthetic-clip\"}\n\nevent: complete\ndata: {}\n\n" };
            ([ ("content-type", "text/event-stream") ], body)
        }))
        .route("/__api/clips/auth-user", get(|| async {
            Json(json!({"clips":[{"id":"synthetic-clip","audio_url":"https://media.example/music.m4a","status":"complete"}]}))
        }))
        .with_state(calls.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let document = json!({"providers":[{"id":"flowmusic-web","preset":"producer","base_url":base,
        "explicit_credentials_only":true,"credentials":[{"id":"synthetic-account","api_key":"synthetic-flowmusic-session"}],
        "supported_models":["producer:standard"],"execution_mode":"direct_http",
        "call_flow":{"call_adapter":"auto","fingerprint_adapter":"session","special_adapter":"flowmusic_web"}}],
        "model_routes":[{"pattern":"producer:standard","provider_ids":["flowmusic-web"]}],"aliases":{}});
    let mut fixture =
        support::ConsoleStateFixture::new(serde_json::from_value(document).unwrap(), true);
    let root = std::env::temp_dir().join(format!("flowmusic-ingress-{}", uuid::Uuid::new_v4()));
    Arc::get_mut(&mut fixture.state).unwrap().local_runtime =
        Some(LocalRuntime::open(&root).await.unwrap());
    let response = build_router(fixture.state.clone())
        .oneshot(
            Request::post("/v1/music/generations")
                .header("authorization", "Bearer public-api-contract-secret")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"model":"producer:standard","prompt":"Synthetic instrumental"})
                        .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let body = support::parse_json(response).await;
    if fail_stream {
        assert!(status.is_server_error(), "{status}: {body}");
        assert!(body["data"].is_null());
    } else {
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["provider"], "flowmusic.app");
        assert_eq!(body["media_completed"], true);
        assert_eq!(body["data"][0]["url"], "https://media.example/music.m4a");
    }
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    fixture.state.local_runtime.as_ref().unwrap().close().await;
    server.abort();
    let _ = server.await;
}

#[tokio::test]
async fn public_music_ingress_completes_one_http_generation() {
    exercise(false).await;
}

#[tokio::test]
async fn accepted_music_stream_failure_does_not_resubmit_generation() {
    exercise(true).await;
}
