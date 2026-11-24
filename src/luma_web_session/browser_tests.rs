//! Opt-in production Rust -> Node -> real browser -> private persistence loopback contract.
use super::{
    tests::{config, payload},
    SessionStore,
};
use crate::{
    protocol::canonical::{CanonicalRelayRequest, EndpointKind, ProtocolFamily},
    upstream::client::UpstreamClient,
};
use axum::{
    extract::{Request, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::any,
    Router,
};
use serde_json::json;
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct Fixture {
    calls: Mutex<Vec<(String, String)>>,
}
async fn handle(State(state): State<Arc<Fixture>>, request: Request) -> Response {
    let path = request.uri().path();
    if path == "/favicon.ico" {
        return StatusCode::NOT_FOUND.into_response();
    }
    let cookie = request
        .headers()
        .get(header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    state
        .calls
        .lock()
        .unwrap()
        .push((path.into(), cookie.into()));
    if path == "/board/fixture" {
        return (
            [(header::CONTENT_TYPE, "text/html")],
            "<!doctype html><title>Fixture</title>",
        )
            .into_response();
    }
    if path.ends_with("/signature") {
        return ([(header::SET_COOKIE, "wos-session=after-signature; Path=/; HttpOnly; Secure; SameSite=Lax; Max-Age=3600")],
            axum::Json(json!({"ws_token":"synthetic", "cdn_url":"https://asset.fixture.invalid", "query_params":"v=1"}))).into_response();
    }
    if path.ends_with("/events/stream/grip") {
        let stream = futures::stream::pending::<Result<axum::body::Bytes, std::io::Error>>();
        return (
            [(header::CONTENT_TYPE, "text/event-stream")],
            axum::body::Body::from_stream(stream),
        )
            .into_response();
    }
    if path.ends_with("/actions") {
        return (
            StatusCode::PAYMENT_REQUIRED,
            [(
                header::SET_COOKIE,
                "wos-session=after-refusal; Path=/; HttpOnly; Secure; SameSite=Lax; Max-Age=3600",
            )],
            axum::Json(json!({"message":"Not enough credits"})),
        )
            .into_response();
    }
    StatusCode::NOT_FOUND.into_response()
}

#[tokio::test]
#[ignore = "requires installed Chromium; loopback only, no live credentials"]
async fn luma_web_session_real_browser_rust_failure_persists_and_reopens() {
    let fixture = Arc::new(Fixture::default());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    let app = Router::new()
        .fallback(any(handle))
        .with_state(fixture.clone());
    let server = tokio::spawn(async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(async {
                let _ = stopped.await;
            })
            .await
            .unwrap();
    });
    let config = config();
    let mut p = payload();
    p.base_url = format!("http://{address}");
    let client = UpstreamClient::new(30);
    assert!(client
        .luma_web_sessions
        .set(SessionStore::new(&config, None).unwrap())
        .is_ok());
    client
        .maintain_luma_web_session("provider", &p)
        .await
        .unwrap();
    assert_eq!(fixture.calls.lock().unwrap().len(), 2);
    drop(client);
    let client = UpstreamClient::new(30);
    assert!(client
        .luma_web_sessions
        .set(SessionStore::new(&config, None).unwrap())
        .is_ok());
    let request = CanonicalRelayRequest {
        protocol_family: ProtocolFamily::OpenAi,
        endpoint_kind: EndpointKind::ImagesGenerations,
        requested_model: None,
        stream: false,
        messages: vec![],
        tools: vec![],
        tool_choice: None,
        reasoning: None,
        metadata: None,
        raw_body: json!({"prompt":"synthetic fixture", "action_type":"fixture-action"}),
        previous_response_id: None,
        explicit_session_key: None,
        extra: Default::default(),
    };
    // Explicit action type avoids directory discovery; this is a synthetic refusal, never paid upstream.
    p.extra_body
        .as_mut()
        .unwrap()
        .insert("imageActionType".into(), json!("fixture-action"));
    let error = client
        .execute_lumalabs_media("provider", &p, &request, "fixture", None)
        .await
        .unwrap_err();
    assert_eq!(error.http_status, Some(402));
    assert!(!error.retryable);
    assert!(!error.message.contains("after-refusal"));
    let store = SessionStore::new(&config, None).unwrap();
    let attempt = store.begin("provider", &p, false).await.unwrap().unwrap();
    assert!(attempt.token() == "after-refusal");
    let calls = fixture.calls.lock().unwrap();
    assert!(calls[2].1.contains("wos-session=after-signature"));
    assert_eq!(
        calls
            .iter()
            .filter(|(path, _)| path.ends_with("/actions"))
            .count(),
        1
    );
    drop(calls);
    stop.send(()).unwrap();
    server.await.unwrap();
}
