//! Loopback-only HTTP fixture with bounded observations and joined shutdown.
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    Router,
};
use tokio::{sync::oneshot, task::JoinHandle};

pub(super) struct Server {
    pub url: String,
    seen: Arc<Mutex<Vec<Instant>>>,
    shutdown: Option<oneshot::Sender<()>>,
    task: JoinHandle<()>,
}

#[derive(Clone)]
struct Reply {
    seen: Arc<Mutex<Vec<Instant>>>,
    status: StatusCode,
    headers: HeaderMap,
    body: &'static str,
}

async fn respond(State(reply): State<Reply>) -> (StatusCode, HeaderMap, &'static str) {
    let mut seen = reply.seen.lock().unwrap();
    assert!(seen.len() < 16, "unexpected unbounded transport calls");
    seen.push(Instant::now());
    (reply.status, reply.headers, reply.body)
}

impl Server {
    pub async fn start(
        status: StatusCode,
        headers: &[(&'static str, &'static str)],
        body: &'static str,
    ) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let seen = Arc::new(Mutex::new(Vec::new()));
        let headers = headers
            .iter()
            .map(|(name, value)| {
                (
                    axum::http::HeaderName::from_static(name),
                    axum::http::HeaderValue::from_static(value),
                )
            })
            .collect();
        let app = Router::new().fallback(respond).with_state(Reply {
            seen: Arc::clone(&seen),
            status,
            headers,
            body,
        });
        let (shutdown, receiver) = oneshot::channel();
        let task = tokio::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(async {
                    let _ = receiver.await;
                })
                .await
                .unwrap();
        });
        Self {
            url,
            seen,
            shutdown: Some(shutdown),
            task,
        }
    }

    pub fn calls(&self) -> Vec<Instant> {
        self.seen.lock().unwrap().clone()
    }

    pub async fn finish(mut self) {
        let _ = self.shutdown.take().unwrap().send(());
        tokio::time::timeout(Duration::from_secs(2), &mut self.task)
            .await
            .expect("fixture shutdown")
            .unwrap();
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        self.task.abort();
    }
}
