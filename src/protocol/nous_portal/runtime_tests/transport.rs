//! Deliberately unsafe ambient defaults and redirect policy cannot escape the product-bound sender.
use super::*;
use axum::{body::Body, extract::State, http::Request, response::Response, Router};
use std::sync::{Arc, Mutex};
type Calls = Arc<Mutex<Vec<(String, axum::http::HeaderMap)>>>;
#[derive(Clone)]
struct Reply {
    origin: String,
    calls: Calls,
    redirect: bool,
}
struct Peer {
    origin: String,
    calls: Calls,
    task: Option<tokio::task::JoinHandle<()>>,
}
impl Peer {
    async fn new(redirect: bool) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let calls = Arc::new(Mutex::new(Vec::new()));
        let app = Router::new().fallback(handler).with_state(Reply {
            origin: origin.clone(),
            calls: calls.clone(),
            redirect,
        });
        let task = Some(tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap()
        }));
        Self {
            origin,
            calls,
            task,
        }
    }
    async fn stop(mut self) {
        let task = self.task.take().unwrap();
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
    }
}
impl Drop for Peer {
    fn drop(&mut self) {
        if let Some(task) = &self.task {
            task.abort();
        }
    }
}
async fn handler(State(reply): State<Reply>, request: Request<Body>) -> Response {
    assert_eq!(request.method(), axum::http::Method::POST);
    let (parts, body) = request.into_parts();
    let _: serde_json::Value =
        serde_json::from_slice(&axum::body::to_bytes(body, 65536).await.unwrap()).unwrap();
    reply
        .calls
        .lock()
        .unwrap()
        .push((parts.uri.path().into(), parts.headers));
    Response::builder()
        .status(if reply.redirect { 307 } else { 200 })
        .header("location", format!("{}/escaped", reply.origin))
        .header("content-type", "application/json")
        .body(Body::from("{}"))
        .unwrap()
}
#[tokio::test]
async fn nous_actual_sender_strips_ambient_defaults_and_refuses_redirect() {
    for target in [
        "https://inference-api.nousresearch.com/v1",
        "https://welcome-api.nousresearch.com/v1",
    ] {
        for redirect in [false, true] {
            let peer = Peer::new(redirect).await;
            let payload: ProviderAccountPayload = serde_json::from_value(json!({
                "adapter":"openai_compatible","auth_mode":"nous_portal_oauth","base_url":target,"api_key":"synthetic-access"
            })).unwrap();
            let request = openai::normalize_chat_completions(
                json!({"messages":[{"role":"user","content":"hello"}]}),
            )
            .unwrap();
            let plan =
                UpstreamClient::build_request_plan(&payload, &request, "org/model", false).unwrap();
            let mut defaults = rquest::header::HeaderMap::new();
            for name in ["cookie", "x-api-key", "originator", "chatgpt-account-id"] {
                defaults.insert(
                    HeaderName::from_bytes(name.as_bytes()).unwrap(),
                    HeaderValue::from_static("ambient-secret"),
                );
            }
            let mut client = UpstreamClient::new(3);
            client.http = rquest::Client::builder()
                .no_proxy()
                .default_headers(defaults)
                .redirect(rquest::redirect::Policy::limited(3))
                .build()
                .unwrap();
            let headers = build_inference_headers(&payload, &request, "org/model", None);
            super::super::test_support::origin(peer.origin.clone(), async {
                let response = client
                    .send_profiled_plan(&payload, &plan, headers)
                    .unwrap()
                    .send()
                    .await
                    .unwrap();
                assert_eq!(response.status().as_u16(), if redirect { 307 } else { 200 });
                let _ = response.text().await.unwrap();
            })
            .await;
            let calls = peer.calls.lock().unwrap().clone();
            assert_eq!(
                calls.len(),
                1,
                "a redirect must not replay the bearer or request"
            );
            assert_eq!(calls[0].0, "/v1/chat/completions");
            assert_eq!(calls[0].1["authorization"], "Bearer synthetic-access");
            assert_eq!(calls[0].1["accept"], "application/json");
            for name in ["cookie", "x-api-key", "originator", "chatgpt-account-id"] {
                assert!(!calls[0].1.contains_key(name), "ambient {name} escaped");
            }
            peer.stop().await;
        }
    }
}
