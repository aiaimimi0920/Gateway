//! A local origin/proxy never forwards CONNECT or HTTP requests to a real upstream.
use axum::{
    body::Body,
    extract::State,
    http::{Request, StatusCode},
    response::Response,
    Router,
};
use serde_json::{json, Value};
use std::{
    future::IntoFuture,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

#[derive(Clone, Debug)]
pub(super) struct Call {
    pub uri: String,
    pub authorization: String,
    pub user_agent: String,
    pub body: Value,
}
#[derive(Clone, Default)]
struct WireState {
    calls: Arc<Mutex<Vec<Call>>>,
    connections: Arc<AtomicUsize>,
    closed: Arc<AtomicUsize>,
}
pub(super) struct Server {
    pub origin: String,
    state: WireState,
    task: tokio::task::JoinHandle<()>,
}
impl Server {
    pub async fn start() -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let state = WireState::default();
        let app = Router::new().fallback(handler).with_state(state.clone());
        let task = tokio::spawn(async move {
            axum::serve(listener, app).into_future().await.unwrap();
        });
        Self {
            origin,
            state,
            task,
        }
    }
    pub fn client(&self) -> rquest::Client {
        // Even if a regression removes the URL seam, HTTPS can only reach this non-forwarding proxy.
        rquest::Client::builder()
            .no_proxy()
            .proxy(rquest::Proxy::all(&self.origin).unwrap())
            .timeout(Duration::from_secs(1))
            .build()
            .unwrap()
    }
    pub fn calls(&self) -> Vec<Call> {
        self.state.calls.lock().unwrap().clone()
    }
    pub fn attempts(&self) -> usize {
        self.state.connections.load(Ordering::SeqCst)
    }
    pub fn closed(&self) -> usize {
        self.state.closed.load(Ordering::SeqCst)
    }
    pub async fn stop(self) {
        self.task.abort();
        assert!(self.task.await.unwrap_err().is_cancelled());
    }
}

async fn handler(State(state): State<WireState>, request: Request<Body>) -> Response {
    state.connections.fetch_add(1, Ordering::SeqCst);
    if request.method() != axum::http::Method::POST {
        return Response::builder()
            .status(StatusCode::BAD_GATEWAY)
            .body(Body::empty())
            .unwrap();
    }
    let (parts, body) = request.into_parts();
    // A rewritten plan still goes through the real HTTP sender and this actual local TCP proxy.
    assert_eq!(parts.uri.scheme_str(), Some("http"));
    assert_eq!(parts.uri.host(), Some("127.0.0.1"));
    let body: Value =
        serde_json::from_slice(&axum::body::to_bytes(body, 32768).await.unwrap()).unwrap();
    let authorization = parts.headers["authorization"].to_str().unwrap().to_string();
    state.calls.lock().unwrap().push(Call {
        uri: parts.uri.path().into(),
        authorization: authorization.clone(),
        user_agent: parts
            .headers
            .get("user-agent")
            .unwrap()
            .to_str()
            .unwrap()
            .into(),
        body: body.clone(),
    });
    let prompt = body["messages"][0]["content"].as_str().unwrap();
    let mut status = StatusCode::OK;
    let mut value = completed("OK");
    let mut builder = Response::builder().header("content-type", "application/json");
    match prompt {
        "hold-body" => {
            let probe = CloseProbe(state.closed.clone());
            let body = futures::stream::unfold((false, probe), |(sent, probe)| async move {
                if sent {
                    std::future::pending().await
                } else {
                    Some((
                        Ok::<_, std::io::Error>(bytes::Bytes::from_static(b"{\"choices\":")),
                        (true, probe),
                    ))
                }
            });
            return builder.body(Body::from_stream(body)).unwrap();
        }
        "401" => status = StatusCode::UNAUTHORIZED,
        "403" => status = StatusCode::FORBIDDEN,
        "429" => status = StatusCode::TOO_MANY_REQUESTS,
        "redirect" => {
            status = StatusCode::TEMPORARY_REDIRECT;
            builder = builder.header("location", format!("{}/leak", parts.uri));
        }
        "invalid-json" => {
            return builder
                .body(Body::from("<html>synthetic-exact-key</html>"))
                .unwrap()
        }
        "invalid-utf8" => return builder.body(Body::from(vec![0xff, 0xfe])).unwrap(),
        "interrupted" => {
            let stream = futures::stream::iter(vec![
                Ok(bytes::Bytes::from_static(b"{\"choices\":")),
                Err(std::io::Error::other("synthetic-exact-key")),
            ]);
            return builder.body(Body::from_stream(stream)).unwrap();
        }
        "empty" => value = completed(" "),
        "missing-stop" => {
            value["choices"][0]
                .as_object_mut()
                .unwrap()
                .remove("finish_reason");
        }
        "length" => value["choices"][0]["finish_reason"] = json!("length"),
        "filtered" => value["choices"][0]["finish_reason"] = json!("content_filter"),
        "wrong-role" => value["choices"][0]["message"]["role"] = json!("user"),
        "tools" => value["choices"][0]["message"]["tool_calls"] = json!([{"id":"tool"}]),
        "refusal" => value["choices"][0]["message"]["refusal"] = json!("denied"),
        "error-envelope" => value["error"] = json!({"message":"synthetic-exact-key"}),
        "oversize" => value = completed(&"x".repeat(65537)),
        "echo" => {
            value = completed(&format!(
                "OK {}",
                authorization.strip_prefix("Bearer ").unwrap()
            ))
        }
        "slow" => tokio::time::sleep(Duration::from_secs(2)).await,
        _ => {}
    }
    if !status.is_success() {
        value = json!({"error":authorization});
    }
    builder
        .status(status)
        .body(Body::from(value.to_string()))
        .unwrap()
}
struct CloseProbe(Arc<AtomicUsize>);
impl Drop for CloseProbe {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

pub(super) fn completed(text: &str) -> Value {
    json!({"choices":[{"message":{"role":"assistant","content":text},"finish_reason":"stop"}],
        "usage":{"prompt_tokens":8,"completion_tokens":1,"total_tokens":9}})
}
