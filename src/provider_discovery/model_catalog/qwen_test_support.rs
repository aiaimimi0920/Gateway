//! Synthetic keys and random loopback only; the real descriptor remains bound to its workspace.
use super::*;
use axum::{
    body::Body,
    extract::State,
    http::{HeaderMap, Uri},
    response::Response,
    routing::get,
    Router,
};
use serde_json::json;
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

pub const BASE: &str = "https://workspace.ap-southeast-1.maas.aliyuncs.com/compatible-mode/v1";
pub type Calls = Arc<Mutex<Vec<(HeaderMap, Uri)>>>;

pub fn row(id: usize) -> Value {
    json!({"model":format!("qwen-contract-{id}"),"provider":"qwen","capabilities":["TG"],
        "inference_metadata":{"request_modality":["Text"],"response_modality":["Text"]}})
}

pub fn page(number: usize, total: usize, rows: Vec<Value>) -> Value {
    json!({"success":true,"code":null,"output":{"total":total,"page_no":number,
        "page_size":20,"models":rows}})
}

pub fn reply(value: Value) -> Response {
    Response::builder()
        .header("content-type", "application/json")
        .body(Body::from(value.to_string()))
        .unwrap()
}

pub struct Server {
    pub calls: Calls,
    pub(super) url: String,
    task: tokio::task::JoinHandle<()>,
}

impl Server {
    pub async fn start(replies: Vec<Response>) -> Self {
        let calls = Calls::default();
        let replies = Arc::new(Mutex::new(VecDeque::from(replies)));
        let app = Router::new()
            .route("/api/v1/models", get(respond))
            .with_state((calls.clone(), replies));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/api/v1/models", listener.local_addr().unwrap());
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        Self { calls, url, task }
    }

    pub async fn fetch(&self) -> Result<ModelCatalog, GatewayError> {
        let client = rquest::Client::builder()
            .no_proxy()
            .redirect(rquest::redirect::Policy::none())
            .build()
            .unwrap();
        fetch_from(
            &client,
            &self.url,
            "synthetic-qwen-key",
            &QwenEndpoint::parse(BASE).unwrap(),
        )
        .await
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}

type ServerState = (Calls, Arc<Mutex<VecDeque<Response>>>);
async fn respond(
    State((calls, replies)): State<ServerState>,
    headers: HeaderMap,
    uri: Uri,
) -> Response {
    calls.lock().unwrap().push((headers, uri));
    replies.lock().unwrap().pop_front().unwrap_or_else(|| {
        Response::builder()
            .status(500)
            .body(Body::from("unexpected extra catalog request"))
            .unwrap()
    })
}
