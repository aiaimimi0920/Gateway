//! Actual sender capture uses a fixed validated Build plan then explicit random-loopback remapping.
use super::*;
use crate::protocol::grok_oauth::business;
use axum::{body::Body, extract::State, http::Request, response::Response, Router};
use std::sync::{Arc, Mutex};

fn payload() -> ProviderAccountPayload {
    serde_json::from_value(
        json!({"adapter":"openai_compatible","auth_mode":business::AUTH_MODE,
        "base_url":crate::protocol::grok_oauth::BUSINESS_BASE_URL,"responses_path":"/responses",
        "api_key":"synthetic-build-only"}),
    )
    .unwrap()
}
#[derive(Clone)]
struct Reply {
    status: u16,
    body: String,
    calls: Arc<Mutex<Vec<(axum::http::HeaderMap, Value)>>>,
}
struct Server {
    origin: String,
    calls: Arc<Mutex<Vec<(axum::http::HeaderMap, Value)>>>,
    task: Option<tokio::task::JoinHandle<()>>,
}
impl Server {
    async fn start(status: u16, body: String) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let calls = Arc::new(Mutex::new(Vec::new()));
        let app = Router::new().fallback(handler).with_state(Reply {
            status,
            body,
            calls: calls.clone(),
        });
        let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        Self {
            origin,
            calls,
            task: Some(task),
        }
    }
    async fn stop(mut self) {
        self.task.as_ref().unwrap().abort();
        let _ = self.task.take().unwrap().await;
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        if let Some(task) = &self.task {
            task.abort();
        }
    }
}
async fn handler(State(reply): State<Reply>, request: Request<Body>) -> Response {
    assert_eq!(request.method(), axum::http::Method::POST);
    assert_eq!(request.uri().path(), "/v1/responses");
    let (parts, body) = request.into_parts();
    let body =
        serde_json::from_slice(&axum::body::to_bytes(body, 64 * 1024).await.unwrap()).unwrap();
    reply.calls.lock().unwrap().push((parts.headers, body));
    Response::builder()
        .status(reply.status)
        .header("content-type", "text/event-stream")
        .header("location", "http://127.0.0.1:9/never-follow")
        .body(Body::from(reply.body))
        .unwrap()
}
fn completed() -> String {
    let response = json!({"id":"build-response","model":"grok-contract","status":"completed",
        "output":[{"type":"message","role":"assistant","content":[{"type":"output_text","text":"OK"}]}],
        "usage":{"input_tokens":3,"output_tokens":1,"total_tokens":4}});
    format!(
        "event: response.completed\ndata: {}\n\n",
        json!({"type":"response.completed","response":response})
    )
}

#[tokio::test]
async fn grok_oauth_buffered_and_stream_sender_use_fixed_wire_and_gateway_identity() {
    for stream in [false, true] {
        let server = Server::start(200, completed()).await;
        let payload = payload();
        let req=openai::normalize_chat_completions(json!({"model":"grok-contract","messages":[{"role":"user","content":"hello"}],"stream":stream})).unwrap();
        let mut client = UpstreamClient::new(3);
        client.http = rquest::Client::builder().no_proxy().build().unwrap();
        let extras = std::collections::HashMap::from([
            ("Originator".into(), "codex_cli_rs".into()),
            ("x-app".into(), "cli".into()),
            (
                "Chatgpt-Account-Id".into(),
                "private-foreign-account".into(),
            ),
        ]);
        grok_oauth_test_support::within(&server.origin, async {
            if stream {
                let UpstreamStreamingResponse::Http(response) = client
                    .execute_stream(&payload, &req, "grok-contract", Some(&extras))
                    .await
                    .unwrap()
                else {
                    panic!("must use product HTTP sender")
                };
                assert!(response
                    .text()
                    .await
                    .unwrap()
                    .contains("response.completed"));
            } else {
                let response = client
                    .execute(&payload, &req, "grok-contract", Some(&extras))
                    .await
                    .unwrap();
                assert!(serde_json::to_string(&response).unwrap().contains("OK"));
            }
        })
        .await;
        let calls = server.calls.lock().unwrap().clone();
        assert_eq!(calls.len(), 1);
        let (headers, body) = &calls[0];
        assert_eq!(headers["authorization"], "Bearer synthetic-build-only");
        assert_eq!(headers["user-agent"], "NeuroGateway/GrokBuild");
        assert_eq!(headers["x-xai-token-auth"], "xai-grok-cli");
        assert_eq!(headers["accept"], "text/event-stream");
        assert!(!headers.contains_key("originator") && !headers.contains_key("x-app"));
        assert!(!headers.contains_key("chatgpt-account-id"));
        assert!(body["input"].is_array() && body.get("messages").is_none());
        assert_eq!(body["stream"], true);
        assert_eq!(body["model"], "grok-contract");
        server.stop().await;
    }
}

#[tokio::test]
async fn grok_oauth_real_sender_refuses_redirect_and_late_plan_or_auth_header_mutation() {
    let server = Server::start(307, String::new()).await;
    let payload = payload();
    let req = responses::normalize_responses(json!({"input":"hello"})).unwrap();
    let plan = UpstreamClient::build_request_plan(&payload, &req, "grok-contract", true).unwrap();
    let mut client = UpstreamClient::new(3);
    client.http = rquest::Client::builder().no_proxy().build().unwrap();
    let headers = build_inference_headers(&payload, &req, "grok-contract", None);
    grok_oauth_test_support::within(&server.origin, async {
        let response = client
            .send_profiled_plan(&payload, &plan, headers.clone())
            .unwrap()
            .send()
            .await
            .unwrap();
        assert_eq!(response.status().as_u16(), 307);
    })
    .await;
    for name in ["cookie", "host", "proxy-authorization", "x-api-key"] {
        let mut bad = headers.clone();
        bad.insert(
            HeaderName::from_bytes(name.as_bytes()).unwrap(),
            HeaderValue::from_static("private"),
        );
        assert!(client.send_profiled_plan(&payload, &plan, bad).is_err());
    }
    let mut bad = headers;
    bad.insert(
        "authorization",
        HeaderValue::from_static("Bearer foreign-token"),
    );
    assert!(client.send_profiled_plan(&payload, &plan, bad).is_err());
    let mut bad = plan;
    bad.url = "https://api.x.ai/v1/responses".into();
    assert!(client
        .send_profiled_plan(
            &payload,
            &bad,
            build_inference_headers(&payload, &req, "grok-contract", None)
        )
        .is_err());
    assert_eq!(server.calls.lock().unwrap().len(), 1);
    server.stop().await;
}
