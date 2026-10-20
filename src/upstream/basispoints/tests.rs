//! Real HTTP request/response behavior, with fixture credentials sent only to owned loopback.
use super::*;
use axum::{extract::Request, response::IntoResponse, routing::any, Router};
use serde_json::json;
use std::sync::{Arc, Mutex};

struct Server {
    url: String,
    seen: Arc<Mutex<Vec<(axum::http::HeaderMap, Value)>>>,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl Server {
    async fn start(status: u16, body: String) -> Self {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let capture = seen.clone();
        let app = Router::new().fallback(any(move |req: Request| {
            let seen = capture.clone();
            let body = body.clone();
            async move {
                let headers = req.headers().clone();
                let request = axum::body::to_bytes(req.into_body(), 1_048_576)
                    .await
                    .unwrap();
                seen.lock()
                    .unwrap()
                    .push((headers, serde_json::from_slice(&request).unwrap()));
                (
                    axum::http::StatusCode::from_u16(status).unwrap(),
                    [
                        ("Content-Type", "application/json"),
                        ("Location", "/unexpected-redirect"),
                    ],
                    body,
                )
                    .into_response()
            }
        }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!(
            "http://{}/basispoints/api/responses",
            listener.local_addr().unwrap()
        );
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        Self { url, seen, task }
    }
}
fn payload() -> ProviderAccountPayload {
    serde_json::from_value(json!({"adapter":wire::ADAPTER,"api_key":"fixture-access","base_url":"https://chatgpt.com/backend-api/codex",
        "headers":{"Chatgpt-Account-Id":"fixture-account","Cookie":"never-forward-me","Authorization":"never-forward-me"}})).unwrap()
}
fn request() -> CanonicalRelayRequest {
    crate::protocol::responses::normalize_responses(json!({"model":"gpt-fixture","input":"hello"}))
        .unwrap()
}

#[tokio::test]
async fn real_http_uses_oauth_product_profile_and_aggregates_mislabelled_sse() {
    let response = json!({"id":"r","model":"gpt-fixture","status":"completed","output":[
        {"id":"reason","type":"reasoning","summary":[],"encrypted_content":"opaque"},
        {"id":"message","type":"message","role":"assistant","content":[{"type":"output_text","text":"answer"}]}]});
    let server = Server::start(
        200,
        format!(
            "data: {}\n\n",
            json!({"type":"response.completed","response":response})
        ),
    )
    .await;
    let http = rquest::Client::builder().no_proxy().build().unwrap();
    let result = exchange_at(
        &http,
        Duration::from_secs(2),
        &payload(),
        &request(),
        "gpt-fixture",
        &server.url,
    )
    .await
    .unwrap();
    assert_eq!(result, response);
    let seen = server.seen.lock().unwrap();
    assert_eq!(seen.len(), 1);
    let (headers, body) = &seen[0];
    assert_eq!(headers["authorization"], "Bearer fixture-access");
    assert_eq!(headers["chatgpt-account-id"], "fixture-account");
    assert_eq!(headers["x-basispoints-auth-mode"], "chatgpt");
    assert_eq!(headers["origin"], "https://bps.openai.com");
    assert_eq!(
        headers["x-openai-internal-basispoints-client-product"],
        "basispoints-excel-plugin"
    );
    assert!(headers.get("cookie").is_none());
    assert_eq!(body["store"], false);
    assert_eq!(body["model_selection"], "explicit");
}

#[tokio::test]
async fn redirects_and_partial_streams_never_become_success_or_forward_credentials() {
    let http = rquest::Client::builder().no_proxy().build().unwrap();
    for (status, body) in [
        (302, "redirect"),
        (
            200,
            "data: {\"type\":\"response.output_text.delta\",\"delta\":\"partial\"}\n\n",
        ),
    ] {
        let server = Server::start(status, body.into()).await;
        assert!(exchange_at(
            &http,
            Duration::from_secs(2),
            &payload(),
            &request(),
            "gpt-fixture",
            &server.url
        )
        .await
        .is_err());
        assert_eq!(server.seen.lock().unwrap().len(), 1);
    }
    let mut missing = payload();
    missing.headers.clear();
    let server = Server::start(200, "{}".into()).await;
    assert!(exchange_at(
        &http,
        Duration::from_secs(2),
        &missing,
        &request(),
        "gpt-fixture",
        &server.url
    )
    .await
    .is_err());
    assert!(server.seen.lock().unwrap().is_empty());
}
