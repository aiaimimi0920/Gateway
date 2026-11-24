//! Non-forwarding native peer: arbitrary byte fragmentation and observable source drop.
use super::*;
use axum::{extract::State, http::HeaderMap, response::Response, Router};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Mutex,
};
pub(super) type Calls = Vec<(String, HeaderMap, Value)>;
#[derive(Clone)]
struct Reply {
    family: &'static str,
    tools: bool,
    hold: bool,
    status: u16,
    retry_after: Arc<Mutex<Option<String>>>,
    oauth_hold: Arc<std::sync::atomic::AtomicBool>,
    oauth_entered: Arc<tokio::sync::Notify>,
    oauth_release: Arc<tokio::sync::Semaphore>,
    disconnect: Arc<tokio::sync::Notify>,
    calls: Arc<Mutex<Calls>>,
    oauth: Arc<Mutex<Vec<(HeaderMap, Vec<u8>)>>>,
    token: Arc<Mutex<(u16, Vec<u8>)>>,
    closed: Arc<AtomicUsize>,
    raw: Option<Vec<Vec<u8>>>,
    response_id_prefix: Option<String>,
}
pub(crate) struct Peer {
    pub origin: String,
    pub calls: Arc<Mutex<Calls>>,
    pub oauth: Arc<Mutex<Vec<(HeaderMap, Vec<u8>)>>>,
    pub token: Arc<Mutex<(u16, Vec<u8>)>>,
    pub retry_after: Arc<Mutex<Option<String>>>,
    pub oauth_hold: Arc<std::sync::atomic::AtomicBool>,
    oauth_entered: Arc<tokio::sync::Notify>,
    pub oauth_release: Arc<tokio::sync::Semaphore>,
    pub disconnect: Arc<tokio::sync::Notify>,
    closed: Arc<AtomicUsize>,
    task: Option<tokio::task::JoinHandle<()>>,
}
impl Peer {
    pub async fn new(family: &'static str, tools: bool, hold: bool, status: u16) -> Self {
        Self::serve(family, tools, hold, status, None, None).await
    }
    pub async fn raw(bytes: Vec<u8>) -> Self {
        Self::serve("gemini", false, false, 200, Some(vec![bytes]), None).await
    }
    pub async fn raw_held(bytes: Vec<u8>) -> Self {
        Self::serve("gemini", false, true, 200, Some(vec![bytes]), None).await
    }
    pub async fn raw_sequence(replies: Vec<Vec<u8>>) -> Self {
        assert!(!replies.is_empty());
        Self::serve("gemini", false, false, 200, Some(replies), None).await
    }
    pub async fn responses_with_unique_ids(prefix: String) -> Self {
        Self::serve("responses", false, false, 200, None, Some(prefix)).await
    }
    pub async fn stored_responses(id: String) -> Self {
        let reply = |value: Value| value.to_string().into_bytes();
        Self::serve("stored_responses", false, false, 200, Some(vec![
            reply(json!({"id":id,"object":"response","status":"queued","output":[],"store":true})),
            reply(json!({"id":id,"object":"response","status":"completed","output":[],"store":true,
                "usage":{"input_tokens":2,"output_tokens":3,"total_tokens":5}})),
        ]), None).await
    }
    pub async fn chat_with_unique_ids(prefix: String) -> Self {
        Self::serve("chat", false, false, 200, None, Some(prefix)).await
    }
    async fn serve(
        family: &'static str,
        tools: bool,
        hold: bool,
        status: u16,
        raw: Option<Vec<Vec<u8>>>,
        response_id_prefix: Option<String>,
    ) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let calls = Arc::new(Mutex::new(Vec::new()));
        let oauth = Arc::new(Mutex::new(Vec::new()));
        let token = Arc::new(Mutex::new((
            200,
            json!({"access_token":"synthetic-sap-access","token_type":"bearer","expires_in":3600})
                .to_string()
                .into_bytes(),
        )));
        let closed = Arc::new(AtomicUsize::new(0));
        let retry_after = Arc::new(Mutex::new(None));
        let oauth_hold = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let oauth_entered = Arc::new(tokio::sync::Notify::new());
        let oauth_release = Arc::new(tokio::sync::Semaphore::new(0));
        let disconnect = Arc::new(tokio::sync::Notify::new());
        let app = Router::new().fallback(handler).with_state(Reply {
            family,
            tools,
            hold,
            status,
            retry_after: retry_after.clone(),
            oauth_hold: oauth_hold.clone(),
            oauth_entered: oauth_entered.clone(),
            oauth_release: oauth_release.clone(),
            disconnect: disconnect.clone(),
            calls: calls.clone(),
            oauth: oauth.clone(),
            token: token.clone(),
            closed: closed.clone(),
            raw,
            response_id_prefix,
        });
        let task = Some(tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap()
        }));
        Self {
            origin,
            calls,
            oauth,
            token,
            retry_after,
            oauth_hold,
            oauth_entered,
            oauth_release,
            disconnect,
            closed,
            task,
        }
    }
    pub fn client(&self) -> rquest::Client {
        // Any missing fixed-target test seam reaches only this rejecting loopback proxy.
        rquest::Client::builder()
            .no_proxy()
            .proxy(rquest::Proxy::all(&self.origin).unwrap())
            .timeout(Duration::from_secs(5))
            .redirect(rquest::redirect::Policy::limited(3))
            .build()
            .unwrap()
    }
    pub async fn wait_calls(&self) {
        tokio::time::timeout(Duration::from_secs(3), async {
            while self.calls.lock().unwrap().is_empty() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
    }
    pub async fn wait_oauth(&self) {
        tokio::time::timeout(Duration::from_secs(3), self.oauth_entered.notified())
            .await
            .expect("OAuth handler must reach the release barrier");
    }
    pub async fn wait_closed(&self) {
        tokio::time::timeout(Duration::from_secs(3), async {
            while self.closed.load(Ordering::SeqCst) == 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
    }
    pub async fn stop(mut self) {
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
    if (request.method() != axum::http::Method::POST
        && !(reply.family == "stored_responses"
            && request.method() == axum::http::Method::GET
            && request
                .uri()
                .path()
                .starts_with("/v2/inference/deployments/deployment1/responses/")))
        || request.uri().scheme_str().is_some_and(|s| s != "http")
    {
        return Response::builder().status(502).body(Body::empty()).unwrap();
    }
    let (parts, body) = request.into_parts();
    let bytes = axum::body::to_bytes(body, 128 * 1024).await.unwrap();
    if parts.uri.path() == "/oauth/token" {
        reply
            .oauth
            .lock()
            .unwrap()
            .push((parts.headers, bytes.to_vec()));
        if reply.oauth_hold.load(Ordering::SeqCst) {
            // Explicit release lets a test change live eligibility before token delivery.
            reply.oauth_entered.notify_one();
            reply.oauth_release.acquire().await.unwrap().forget();
        }
        let token = reply.token.lock().unwrap().clone();
        let mut response = Response::builder()
            .status(token.0)
            .header("content-type", "application/json")
            .header("location", "https://never-follow.invalid/secret");
        if let Some(value) = reply.retry_after.lock().unwrap().as_ref() {
            response = response.header("retry-after", value);
        }
        return response.body(Body::from(token.1)).unwrap();
    }
    let raw: Value = if parts.method == axum::http::Method::GET {
        assert!(bytes.is_empty());
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap()
    };
    let bedrock_stream = parts.uri.path().ends_with("/converse-stream");
    let stream = raw["stream"] == true
        || raw["config"]["stream"]["enabled"] == true
        || parts.uri.path().ends_with(":streamGenerateContent");
    let call_number = {
        let mut calls = reply.calls.lock().unwrap();
        calls.push((
            parts.uri.path_and_query().unwrap().to_string(),
            parts.headers,
            raw,
        ));
        calls.len()
    };
    let bytes = if let Some(mut raw) = reply.raw {
        let index = (call_number - 1).min(raw.len() - 1);
        raw.swap_remove(index)
    } else if reply.status == 200 {
        let bytes = native::reply(
            reply.family,
            reply.tools && call_number == 1,
            stream,
            !reply.hold,
        );
        if let Some(prefix) = reply.response_id_prefix {
            // PostgreSQL audit response IDs are unique even across independent fixture runs.
            String::from_utf8(bytes)
                .unwrap()
                .replace(
                    if reply.family == "responses" {
                        "native-resp"
                    } else {
                        "native-chat"
                    },
                    &format!("{prefix}-{call_number}"),
                )
                .into_bytes()
        } else {
            bytes
        }
    } else {
        b"synthetic-private-error synthetic-dim-public-access".to_vec()
    };
    let chunks = bytes
        .chunks(7)
        .map(bytes::Bytes::copy_from_slice)
        .collect::<Vec<_>>();
    let body = Body::from_stream(futures::stream::unfold(
        (
            chunks.into_iter(),
            Close(reply.closed),
            reply.hold,
            reply.disconnect,
        ),
        |(mut chunks, probe, hold, disconnect)| async move {
            match chunks.next() {
                Some(bytes) => Some((
                    Ok::<_, std::io::Error>(bytes),
                    (chunks, probe, hold, disconnect),
                )),
                None if hold => {
                    disconnect.notified().await;
                    Some((
                        Err(std::io::Error::new(
                            std::io::ErrorKind::ConnectionReset,
                            "synthetic body disconnect",
                        )),
                        (chunks, probe, false, disconnect),
                    ))
                }
                None => None,
            }
        },
    ));
    let mut response = Response::builder().status(reply.status).header(
        "content-type",
        if bedrock_stream {
            "application/vnd.amazon.eventstream"
        } else if stream {
            "text/event-stream"
        } else {
            "application/json"
        },
    );
    if let Some(value) = reply.retry_after.lock().unwrap().as_ref() {
        response = response.header("retry-after", value);
    }
    response.body(body).unwrap()
}
struct Close(Arc<AtomicUsize>);
impl Drop for Close {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}
