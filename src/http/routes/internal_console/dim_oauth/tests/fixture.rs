//! No external hosts are used; only cfg(test) may redirect the fixed authority to this joined peer.
use super::*;
use crate::console::dim_oauth::{Session, SessionStatus};
pub(super) use crate::protocol::dim_oauth::test_support::origin;
use axum::{
    body::Body,
    http::HeaderMap,
    response::Response,
    routing::{get, post},
    Router,
};
use std::{
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Mutex,
    },
    time::Duration,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub(super) struct Peer {
    pub origin: String,
    pub calls: Arc<Mutex<Vec<(HeaderMap, String)>>>,
    pub userinfo: Arc<AtomicUsize>,
    pub inference: super::wire::Calls,
    pub catalog_status: Arc<AtomicUsize>,
    pub catalog: Arc<Mutex<Value>>,
    pub catalog_reads: Arc<AtomicUsize>,
    pub catalog_blocked: Arc<AtomicBool>,
    pub catalog_release: Arc<tokio::sync::Notify>,
    task: Option<tokio::task::JoinHandle<()>>,
}
impl Peer {
    pub async fn new(expires_in: u64, delay: Duration) -> Self {
        let calls: Arc<Mutex<Vec<(HeaderMap, String)>>> = Arc::default();
        let captured = calls.clone();
        let userinfo = Arc::new(AtomicUsize::new(0));
        let reads = userinfo.clone();
        let issuer = "https://dimagent.cn";
        let inference = super::wire::Calls::default();
        let catalog_status = Arc::new(AtomicUsize::new(200));
        let catalog_reply = catalog_status.clone();
        let catalog = Arc::new(Mutex::new(json!({"data":[
            {"id":"opaque-chat","dim":{"endpoint_type":"openai","tool_call":true}},
            {"id":"opaque-responses","dim":{"endpoint_type":"openai-response","tool_call":true}},
            {"id":"opaque-messages","dim":{"endpoint_type":"anthropic","tool_call":true}},
            {"id":"opaque-gemini","dim":{"endpoint_type":"gemini","tool_call":true}}
        ]})));
        let directory = catalog.clone();
        let catalog_reads = Arc::new(AtomicUsize::new(0));
        let directory_reads = catalog_reads.clone();
        let catalog_blocked = Arc::new(AtomicBool::new(false));
        let blocked = catalog_blocked.clone();
        let catalog_release = Arc::new(tokio::sync::Notify::new());
        let release = catalog_release.clone();
        let metadata = json!({"issuer":issuer,"authorization_endpoint":format!("{issuer}/oauth/authorize"),
            "token_endpoint":format!("{issuer}/oauth/token"),"userinfo_endpoint":format!("{issuer}/oauth/userinfo"),
            "jwks_uri":format!("{issuer}/oauth/jwks.json"),"response_types_supported":["code"],
            "code_challenge_methods_supported":["S256"],"token_endpoint_auth_methods_supported":["none"],
            "id_token_signing_alg_values_supported":["RS256"]});
        let app = Router::new()
            .route("/v1/models", get(move |headers: HeaderMap, axum::extract::OriginalUri(uri): axum::extract::OriginalUri| {
                let (status, rows, reads, blocked, release) = (catalog_reply.clone(), directory.clone(), directory_reads.clone(), blocked.clone(), release.clone()); async move {
                assert_eq!(headers["authorization"], "Bearer synthetic-dim-access");
                assert_eq!(uri.query(), Some("type=dim"));
                reads.fetch_add(1, Ordering::SeqCst);
                if blocked.load(Ordering::SeqCst) { release.notified().await; }
                let value = rows.lock().unwrap().clone();
                (StatusCode::from_u16(status.load(Ordering::SeqCst) as u16).unwrap(), axum::Json(value))
            } }))
            .route("/.well-known/openid-configuration", get(move || { let metadata = metadata.clone(); async move { axum::Json(metadata) } }))
            .route("/oauth/token", post(move |headers: HeaderMap, body: String| {
                let calls = captured.clone(); async move {
                    calls.lock().unwrap().push((headers, body));
                    let tokens = json!({"access_token":"synthetic-dim-access","refresh_token":"synthetic-dim-refresh",
                        "token_type":"Bearer","expires_in":expires_in,"scope":"openid profile email"});
                    let stream = futures::stream::once(async move { tokio::time::sleep(delay).await;
                        Ok::<_, std::convert::Infallible>(bytes::Bytes::from(tokens.to_string())) });
                    Response::builder().header("content-type","application/json").body(Body::from_stream(stream)).unwrap()
                }
            }))
            .route("/oauth/userinfo", get(move |headers: HeaderMap| { let reads = reads.clone(); async move {
                assert_eq!(headers["authorization"], "Bearer synthetic-dim-access"); reads.fetch_add(1, Ordering::SeqCst);
                axum::Json(json!({"sub":"synthetic-subject","uid":"synthetic-user","email":"synthetic@example.invalid"}))
            } })).merge(super::wire::router(inference.clone()));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let task = Some(tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        }));
        Self {
            origin,
            calls,
            userinfo,
            inference,
            catalog_status,
            catalog,
            catalog_reads,
            catalog_blocked,
            catalog_release,
            task,
        }
    }
    pub fn count(&self) -> usize {
        self.calls.lock().unwrap().len()
    }
    pub async fn wait_called(&self) {
        tokio::time::timeout(Duration::from_secs(3), async {
            while self.count() == 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
    }
    pub async fn stop(mut self) {
        self.catalog_blocked.store(false, Ordering::SeqCst);
        self.catalog_release.notify_waiters();
        let task = self.task.take().unwrap();
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
    }
}
impl Drop for Peer {
    fn drop(&mut self) {
        self.catalog_blocked.store(false, Ordering::SeqCst);
        self.catalog_release.notify_waiters();
        if let Some(task) = &self.task {
            task.abort();
        }
    }
}
pub(super) fn create_body(state: &AppState) -> Value {
    // The test alone selects a free port. Production must receive the explicitly registered URI.
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    json!({"providerId":"dim","expectedRevision":state.route_config.snapshot().revision().id(),
        "site":"china","clientId":"permitted-gateway","scopes":["openid","profile","email"],
        "redirectUri":format!("http://127.0.0.1:{}/oauth/callback", listener.local_addr().unwrap().port())})
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
        .dim_oauth
        .get_owned(view["id"].as_str().unwrap(), &owner)
        .unwrap()
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
pub(super) fn callback(view: &Value) -> String {
    let auth = url::Url::parse(view["authorizationUrl"].as_str().unwrap()).unwrap();
    let query: std::collections::HashMap<_, _> = auth.query_pairs().into_owned().collect();
    assert_eq!(query["redirect_uri"], view["redirectUri"].as_str().unwrap());
    assert_eq!(query["code_challenge_method"], "S256");
    format!(
        "{}?code=synthetic-code&state={}",
        query["redirect_uri"], query["state"]
    )
}
pub(super) async fn action(
    state: &Arc<AppState>,
    view: &Value,
    grant: Option<&str>,
    name: &str,
    callback: Option<&str>,
    peer: &Peer,
) -> axum::response::Response {
    let mut body = json!({"expectedRevision":view["expectedRevision"]});
    if let Some(callback) = callback {
        body["callbackUrl"] = json!(callback);
    }
    origin(
        peer.origin.clone(),
        call(
            state,
            &format!("{ROOT}/{}/{name}", view["id"].as_str().unwrap()),
            body,
            grant,
        ),
    )
    .await
}
pub(super) async fn complete(state: &Arc<AppState>, view: &Value, grant: &str, peer: &Peer) {
    let response = action(
        state,
        view,
        Some(grant),
        "complete",
        Some(&callback(view)),
        peer,
    )
    .await;
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "{}",
        support::parse_json(response).await
    );
    let body = support::parse_json(response).await;
    assert_eq!(body["session"]["status"], "ready");
    assert_redacted(&body);
}
pub(super) async fn deliver(callback: &str) -> String {
    let url = url::Url::parse(callback).unwrap();
    let mut stream =
        tokio::net::TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, url.port().unwrap()))
            .await
            .unwrap();
    stream
        .write_all(
            format!(
                "GET {}?{} HTTP/1.1\r\nHost: 127.0.0.1:{}\r\n\r\n",
                url.path(),
                url.query().unwrap(),
                url.port().unwrap()
            )
            .as_bytes(),
        )
        .await
        .unwrap();
    let mut reply = String::new();
    tokio::time::timeout(Duration::from_secs(3), stream.read_to_string(&mut reply))
        .await
        .unwrap()
        .unwrap();
    reply
}
pub(super) async fn wait_status(session: &Session, status: SessionStatus) {
    tokio::time::timeout(Duration::from_secs(4), async {
        while session.view().status != status {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
}
pub(super) fn assert_redacted(value: &Value) {
    for secret in [
        "synthetic-dim-access",
        "synthetic-dim-refresh",
        "synthetic-unrelated-secret",
    ] {
        assert!(!value.to_string().contains(secret));
    }
}
