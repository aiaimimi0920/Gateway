//! Fixed synthetic catalog, purchased claim and signed model peer; never a real platform.
use super::*;
use axum::{
    body::Bytes,
    extract::State,
    http::{HeaderMap, Method, Uri},
    response::IntoResponse,
    Json, Router,
};
use std::sync::{Arc, Mutex};
pub(super) struct Server {
    pub origin: String,
    stop: Option<tokio::sync::oneshot::Sender<()>>,
    task: Option<tokio::task::JoinHandle<()>>,
}
impl Server {
    pub async fn start(app: Router) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (stop, stopped) = tokio::sync::oneshot::channel();
        let task = tokio::spawn(async move {
            axum::serve(
                listener,
                app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
            )
            .with_graceful_shutdown(async {
                let _ = stopped.await;
            })
            .await
            .unwrap();
        });
        Self {
            origin: format!("http://{address}"),
            stop: Some(stop),
            task: Some(task),
        }
    }
    pub async fn close(mut self) {
        let _ = self.stop.take().unwrap().send(());
        tokio::time::timeout(Duration::from_secs(5), self.task.as_mut().unwrap())
            .await
            .unwrap()
            .unwrap();
        self.task.take();
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        if let Some(task) = &self.task {
            task.abort();
        }
    }
}
pub(super) type Hits = Arc<Mutex<Vec<(Method, String, HeaderMap, Value)>>>;
pub(super) struct Pause {
    path: &'static str,
    visit: usize,
    entered: tokio::sync::Notify,
    released: tokio::sync::Notify,
}
impl Pause {
    pub async fn reached(&self) {
        tokio::time::timeout(Duration::from_secs(3), self.entered.notified())
            .await
            .unwrap();
    }
    pub fn resume(&self) {
        self.released.notify_one();
    }
}
pub(super) struct Peer {
    pub server: Server,
    pub hits: Hits,
    pub mode: Arc<Mutex<String>>,
    pub live: Arc<super::public_sse::Gate>,
    pause: Arc<Mutex<Option<Arc<Pause>>>>,
}
impl Peer {
    pub async fn new() -> Self {
        use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
        let hits = Hits::default();
        let mode = Arc::new(Mutex::new(String::new()));
        let live = Arc::new(super::public_sse::Gate::default());
        let public_key = Arc::new(Mutex::new(None::<String>));
        let pause = Arc::new(Mutex::new(None::<Arc<Pause>>));
        let app = Router::new().fallback(
            |State((hits, mode, public_key, pause, live)): State<(Hits, Arc<Mutex<String>>,Arc<Mutex<Option<String>>>,Arc<Mutex<Option<Arc<Pause>>>>,Arc<super::public_sse::Gate>)>, method: Method, uri: Uri, headers: HeaderMap, bytes: Bytes| async move {
                let body = if bytes.is_empty() { Value::Null } else { serde_json::from_slice(&bytes).unwrap() };
                let visit = { let mut log = hits.lock().unwrap(); assert!(log.len() < 128); log.push((method.clone(), uri.path().into(), headers.clone(), body.clone())); log.iter().filter(|(_,path,_,_)|path == uri.path()).count() };
                let paused = pause.lock().unwrap().clone();
                if let Some(paused) = paused.filter(|p| p.path == uri.path() && p.visit == visit) {
                    paused.entered.notify_one();
                    paused.released.notified().await;
                }
                let value = match (method, uri.path()) {
                    (Method::GET, "/api/v1/freebuff/models") => json!({"protocol":1,"version":"test","issuedAt":1000,"refreshAt":2000,
                        "plansUrl":"https://invalid.test","fetchId":"synthetic-fetch","rows":[{"key":"open","handle":"fbm1.open","displayName":"synthetic",
                        "tagline":"synthetic","warning":null,"badges":[],"multimodal":false,"premium":false,"dataUse":"service","access":"open","sortOrder":0}]}),
                    (Method::POST, "/api/v1/freebuff/device-keys") => { *public_key.lock().unwrap() = Some(body["publicKey"].as_str().unwrap().into()); json!({"keyId":"synthetic-device"}) },
                    (Method::GET, "/api/v1/freebuff/session") => {
                        let price = if mode.lock().unwrap().as_str() == "price_changed" && visit > 1 {11} else {10};
                        json!({"status":"none","freebucks":{"balance":20,"daily":{"remaining":8},"wallet":{"balance":12},"prices":{"open":price}}})
                    },
                    (Method::POST, "/api/v1/freebuff/session/admission") => {
                        assert_eq!(headers["authorization"], "Bearer synthetic-account");
                        assert_eq!(headers["x-freebuff-model"], "fbm1.open");
                        let now = time::OffsetDateTime::now_utc();
                        json!({"status":"active","instanceId":headers["x-freebuff-instance-id"].to_str().unwrap(),"model":"open","accessTier":"full",
                            "admittedAt":(now-time::Duration::seconds(1)).format(&time::format_description::well_known::Rfc3339).unwrap(),
                            "expiresAt":(now+time::Duration::hours(1)).format(&time::format_description::well_known::Rfc3339).unwrap(),"remainingMs":3600000})
                    },
                    (Method::DELETE, "/api/v1/freebuff/session/attempt") => {
                        assert_eq!(headers["authorization"], "Bearer synthetic-account");
                        assert!(headers.contains_key("x-freebuff-device-sig"));
                        assert!(bytes.is_empty());
                        assert!(!headers.contains_key("cookie"));
                        let hash = hex::encode(ring::digest::digest(&ring::digest::SHA256, &bytes));
                        let payload = format!("freebuff-device-v1\nDELETE\n/api/v1/freebuff/session/attempt\n{}\n{hash}\nsynthetic-fetch", headers["x-freebuff-device-ts"].to_str().unwrap());
                        let key = URL_SAFE_NO_PAD.decode(public_key.lock().unwrap().as_ref().unwrap()).unwrap();
                        let signature = URL_SAFE_NO_PAD.decode(headers["x-freebuff-device-sig"].as_bytes()).unwrap();
                        ring::signature::UnparsedPublicKey::new(&ring::signature::ED25519, key).verify(payload.as_bytes(), &signature).unwrap();
                        let log = hits.lock().unwrap();
                        let original = &log.iter().rev().find(|(_,path,_,_)| path == "/api/v1/freebuff/session/admission").unwrap().2;
                        for key in ["x-freebuff-instance-id", "x-freebuff-desktop-attempt-id"] {
                            assert_eq!(headers[key], original[key]);
                        }
                        json!({"status":"ended","freebucksRefund":0})
                    },
                    (Method::POST, "/api/v1/agent-runs") if body["action"] == "START" => {
                        if mode.lock().unwrap().as_str() == "start_unknown" { json!({}) } else { json!({"runId":"synthetic-run"}) }
                    },
                    (Method::POST, "/api/v1/agent-runs") if body["action"] == "FINISH" => {
                        if mode.lock().unwrap().as_str() == "finish_rejected" { return axum::http::StatusCode::BAD_GATEWAY.into_response(); }
                        json!({"ok":true})
                    },
                    (Method::POST, "/api/v1/chat/completions") => {
                        assert_eq!(headers["authorization"], "Bearer synthetic-account");
                        assert!(headers.contains_key("x-freebuff-device-sig"));
                        let hash = hex::encode(ring::digest::digest(&ring::digest::SHA256,&bytes));
                        let payload = format!("freebuff-device-v1\nPOST\n/api/v1/chat/completions\n{}\n{hash}\nsynthetic-fetch",headers["x-freebuff-device-ts"].to_str().unwrap());
                        let key = URL_SAFE_NO_PAD.decode(public_key.lock().unwrap().as_ref().unwrap()).unwrap();
                        let signature = URL_SAFE_NO_PAD.decode(headers["x-freebuff-device-sig"].as_bytes()).unwrap();
                        ring::signature::UnparsedPublicKey::new(&ring::signature::ED25519,key).verify(payload.as_bytes(),&signature).unwrap();
                        assert!(!headers.contains_key("cookie"));
                        assert_eq!(body["model"], "fbm1.open");
                        assert_eq!(body["codebuff_metadata"]["run_id"], "synthetic-run");
                        if let Some(response) = super::cancellation::stream(&mode.lock().unwrap()) {
                            return response;
                        }
                        if let Some(response) = super::public_sse::stream(&mode.lock().unwrap(), live) {
                            return response;
                        }
                        let mut result = super::protocol_json::completion(&mode.lock().unwrap()).unwrap_or_else(completion);
                        match mode.lock().unwrap().as_str() {
                            "invalid" => result["choices"][0]["finish_reason"] = Value::Null,
                            "unknown" => { result.as_object_mut().unwrap().remove("usage"); },
                            _ => {},
                        }
                        result
                    },
                    _ => return axum::http::StatusCode::NOT_FOUND.into_response(),
                };
                Json(value).into_response()
            }).layer(axum::extract::DefaultBodyLimit::max(65536)).with_state((hits.clone(), mode.clone(), public_key, pause.clone(),live.clone()));
        Self {
            server: Server::start(app).await,
            hits,
            mode,
            live,
            pause,
        }
    }
    pub fn pause(&self, path: &'static str, visit: usize) -> Arc<Pause> {
        let pause = Arc::new(Pause {
            path,
            visit,
            entered: Default::default(),
            released: Default::default(),
        });
        *self.pause.lock().unwrap() = Some(pause.clone());
        pause
    }
    pub fn calls(&self, path: &str) -> usize {
        self.hits
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, p, _, _)| p == path)
            .count()
    }
    pub fn finishes(&self) -> Vec<Value> {
        self.hits
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, _, _, b)| b["action"] == "FINISH")
            .map(|(_, _, _, b)| b.clone())
            .collect()
    }
}
pub(super) fn completion() -> Value {
    json!({"id":"native-id","model":"fbm1.open","created":123,"choices":[{"index":0,"message":{"role":"assistant","content":"native answer",
        "reasoning_content":"native reason","reasoning_details":[{"opaque":"keep"}]},"finish_reason":"stop"}],
        "usage":{"prompt_tokens":3,"completion_tokens":4,"total_tokens":7,"cost":0.1,"cost_details":{"upstream_inference_cost":0.02}}})
}
