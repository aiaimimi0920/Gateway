use super::*;
pub(super) use crate::console::nous_oauth::{Session, SessionStatus};
pub(super) use crate::protocol::nous_portal::test_support::origin;
use axum::{
    body::Body,
    http::HeaderMap,
    response::Response,
    routing::{get, post},
    Router,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Mutex,
};
pub(super) fn access(subject: Option<&str>) -> String {
    let mut claims = json!({"exp":4070908800_u64,"scope":"inference:invoke"});
    if let Some(subject) = subject {
        claims["sub"] = json!(subject);
    }
    format!(
        "{}.{}.{}",
        URL_SAFE_NO_PAD.encode(br#"{"alg":"RS256"}"#),
        URL_SAFE_NO_PAD.encode(claims.to_string()),
        URL_SAFE_NO_PAD.encode(b"synthetic-signature")
    )
}
pub(super) struct Peer {
    pub origin: String,
    pub calls: Arc<Mutex<Vec<(HeaderMap, String)>>>,
    pub reply: Arc<Mutex<Value>>,
    pub catalog: Arc<Mutex<Value>>,
    pub catalog_reads: Arc<AtomicUsize>,
    pub release: Arc<tokio::sync::Notify>,
    task: Option<tokio::task::JoinHandle<()>>,
}
impl Peer {
    pub async fn new(blocked: bool, ttl: u64) -> Self {
        let calls: Arc<Mutex<Vec<(HeaderMap, String)>>> = Arc::default();
        let captured = calls.clone();
        let reply = Arc::new(Mutex::new(json!({"access_token":access(Some("subject")),
            "refresh_token":"synthetic-nous-refresh","scope":"inference:invoke","expires_in":3600})));
        let tokens = reply.clone();
        let release = Arc::new(tokio::sync::Notify::new());
        let wait = release.clone();
        let catalog = Arc::new(Mutex::new(json!({"data":[{"id":"org/model"}]})));
        let rows = catalog.clone();
        let catalog_reads = Arc::new(AtomicUsize::new(0));
        let reads = catalog_reads.clone();
        let app = Router::new()
            .route("/api/oauth/device/code", post(move |body: String| async move {
                assert!(body.contains("client_id=permitted-gateway"));
                assert!(body.contains("scope=inference%3Ainvoke"));
                axum::Json(json!({"device_code":"synthetic-device-secret","user_code":"ABCD",
                    "verification_uri":"https://portal.nousresearch.com/device","expires_in":ttl,"interval":1}))
            }))
            .route("/api/oauth/token", post(move |headers: HeaderMap, body: String| {
                let (calls, tokens, wait) = (captured.clone(), tokens.clone(), wait.clone());
                async move {
                    calls.lock().unwrap().push((headers, body));
                    let stream = futures::stream::once(async move {
                        if blocked { wait.notified().await; }
                        Ok::<_, std::convert::Infallible>(bytes::Bytes::from(tokens.lock().unwrap().to_string()))
                    });
                    Response::builder().header("content-type","application/json").body(Body::from_stream(stream)).unwrap()
                }
            }))
            .route("/v1/models", get(move |headers: HeaderMap| {
                let (rows, reads) = (rows.clone(), reads.clone());
                async move {
                    assert_eq!(headers["authorization"], format!("Bearer {}", access(Some("subject"))));
                    reads.fetch_add(1, Ordering::SeqCst);
                    axum::Json(rows.lock().unwrap().clone())
                }
            }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let task = Some(tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        }));
        Self {
            origin,
            calls,
            reply,
            catalog,
            catalog_reads,
            release,
            task,
        }
    }
    pub fn count(&self) -> usize {
        self.calls.lock().unwrap().len()
    }
    pub async fn wait_called(&self) {
        tokio::time::timeout(Duration::from_secs(4), async {
            while self.count() == 0 {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap();
    }
    pub async fn stop(mut self) {
        self.release.notify_waiters();
        let task = self.task.take().unwrap();
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
    }
}
impl Drop for Peer {
    fn drop(&mut self) {
        self.release.notify_waiters();
        if let Some(task) = &self.task {
            task.abort();
        }
    }
}
pub(super) fn body(state: &AppState) -> Value {
    json!({"providerId":"nous","expectedRevision":state.route_config.snapshot().revision().id(),"clientId":"permitted-gateway"})
}
pub(super) async fn create(state: &Arc<AppState>, grant: &str, body: Value, peer: &Peer) -> Value {
    let response = origin(peer.origin.clone(), call(state, ROOT, body, Some(grant))).await;
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "{}",
        support::parse_json(response).await
    );
    let view = support::parse_json(response).await["session"].clone();
    assert_redacted(&view);
    view
}
pub(super) fn session(state: &Arc<AppState>, view: &Value) -> Arc<Session> {
    let (owner, _) = super::super::super::chatgpt_oauth::authorize(
        state,
        &HeaderMap::new(),
        Some(support::MANAGEMENT_TOKEN),
        None,
        false,
    )
    .unwrap();
    state
        .lifecycle
        .nous_oauth
        .get_owned(view["id"].as_str().unwrap(), &owner)
        .unwrap()
}
pub(super) async fn action(
    state: &Arc<AppState>,
    view: &Value,
    grant: Option<&str>,
    name: &str,
    peer: &Peer,
) -> axum::response::Response {
    origin(
        peer.origin.clone(),
        call(
            state,
            &format!("{ROOT}/{}/{name}", view["id"].as_str().unwrap()),
            json!({"expectedRevision":view["expectedRevision"]}),
            grant,
        ),
    )
    .await
}
pub(super) async fn ready(state: &Arc<AppState>, view: &Value, grant: &str, peer: &Peer) {
    assert_eq!(
        action(state, view, Some(grant), "poll", peer)
            .await
            .status(),
        StatusCode::OK
    );
    wait_status(&session(state, view), SessionStatus::Ready).await;
}
pub(super) async fn wait_status(session: &Session, status: SessionStatus) {
    tokio::time::timeout(Duration::from_secs(5), async {
        while session.view().status != status {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap_or_else(|_| {
        let view = session.view();
        panic!(
            "expected {status:?}, got {:?}, error_code={:?}, receipt_received={}",
            view.status, view.error_code, view.receipt_received
        );
    });
}
pub(super) fn assert_redacted(value: &Value) {
    for secret in [
        access(Some("subject")),
        "synthetic-nous-refresh".into(),
        "synthetic-device-secret".into(),
        "synthetic-unrelated-secret".into(),
    ] {
        assert!(!value.to_string().contains(&secret));
    }
}
