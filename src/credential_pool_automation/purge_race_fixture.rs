//! Deterministic WebDAV barriers for archive-versus-purge interleavings.
use axum::{
    body::Body,
    extract::{Request, State},
    response::Response,
    Router,
};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex,
    },
};
use tokio::{sync::Notify, task::JoinHandle};

#[derive(Default)]
struct Data {
    files: Mutex<BTreeMap<String, Vec<u8>>>,
    pause_readback: AtomicBool,
    read_started: Notify,
    read_release: Notify,
    pause_delete: AtomicBool,
    delete_started: Notify,
    delete_release: Notify,
    deletes: AtomicUsize,
    fail_delete_number: AtomicUsize,
}
pub(super) struct PurgeServer {
    pub endpoint: String,
    data: Arc<Data>,
    task: JoinHandle<()>,
}
impl PurgeServer {
    pub async fn start() -> Self {
        let data = Arc::new(Data::default());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let router = Router::new().fallback(serve).with_state(data.clone());
        let task = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        Self {
            endpoint,
            data,
            task,
        }
    }
    pub fn pause_readback(&self) {
        self.data.pause_readback.store(true, Ordering::SeqCst);
    }
    pub async fn wait_readback(&self) {
        self.data.read_started.notified().await;
    }
    pub fn release_readback(&self) {
        self.data.read_release.notify_one();
    }
    pub fn pause_delete(&self) {
        self.data.pause_delete.store(true, Ordering::SeqCst);
    }
    pub async fn wait_delete(&self) {
        self.data.delete_started.notified().await;
    }
    pub fn release_delete(&self) {
        self.data.delete_release.notify_one();
    }
    pub fn delete_count(&self) -> usize {
        self.data.deletes.load(Ordering::SeqCst)
    }
    pub fn fail_second_delete(&self) {
        self.data.fail_delete_number.store(2, Ordering::SeqCst);
    }
    pub fn records(&self) -> Vec<serde_json::Value> {
        self.data
            .files
            .lock()
            .unwrap()
            .values()
            .map(|b| serde_json::from_slice(b).unwrap())
            .collect()
    }
}
impl Drop for PurgeServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}
fn etag(bytes: &[u8]) -> String {
    format!("\"{:x}\"", Sha256::digest(bytes))
}
fn response(status: u16, bytes: Vec<u8>, tag: Option<String>) -> Response {
    let mut builder = Response::builder().status(status);
    if let Some(tag) = tag {
        builder = builder.header("ETag", tag);
    }
    builder.body(Body::from(bytes)).unwrap()
}
async fn serve(State(data): State<Arc<Data>>, request: Request) -> Response {
    let (parts, body) = request.into_parts();
    let path = parts.uri.path().to_owned();
    match parts.method.as_str() {
        "POST" if path == "/driver" => response(200, br#"{"credentials":[],"prune":[{"credential_id":"a","classification":"permanent_auth_failure"}]}"#.to_vec(), None),
        "PUT" => {
            if parts.headers.get("if-none-match").and_then(|v|v.to_str().ok()) != Some("*") { return response(400,vec![],None); }
            let bytes = axum::body::to_bytes(body,1024*1024).await.unwrap().to_vec();
            let mut files = data.files.lock().unwrap();
            if files.contains_key(&path) { return response(412,vec![],None); }
            files.insert(path,bytes); response(201,vec![],None)
        }
        "GET" => {
            let bytes = data.files.lock().unwrap().get(&path).cloned();
            let Some(bytes) = bytes else { return response(404,vec![],None); };
            if data.pause_readback.swap(false,Ordering::SeqCst) {
                data.read_started.notify_one(); data.read_release.notified().await;
            }
            let tag = etag(&bytes); response(200,bytes,Some(tag))
        }
        "PROPFIND" => {
            let files = data.files.lock().unwrap();
            let entries:String = files.keys().filter(|p|p.starts_with(&path)).map(|key|format!("<d:response><d:href>{key}</d:href><d:propstat><d:prop><d:resourcetype/></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response>")).collect();
            response(207,format!("<d:multistatus xmlns:d=\"DAV:\">{entries}</d:multistatus>").into_bytes(),None)
        }
        "DELETE" => {
            let number = data.deletes.fetch_add(1,Ordering::SeqCst)+1;
            if data.pause_delete.swap(false,Ordering::SeqCst) { data.delete_started.notify_one(); data.delete_release.notified().await; }
            if data.fail_delete_number.load(Ordering::SeqCst)==number { return response(500,vec![],None); }
            let mut files = data.files.lock().unwrap();
            let Some(bytes) = files.get(&path) else { return response(404,vec![],None); };
            if parts.headers.get("if-match").and_then(|v|v.to_str().ok()) != Some(etag(bytes).as_str()) { return response(412,vec![],None); }
            files.remove(&path); response(204,vec![],None)
        }
        _ => response(405,vec![],None),
    }
}
