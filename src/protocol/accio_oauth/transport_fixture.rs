//! Loopback-only synthetic OAuth peer; every test owns and joins its server task.
use axum::{body::Body, http::HeaderMap, response::Response, routing::post, Router};
use futures::Stream;
use std::{
    convert::Infallible,
    pin::Pin,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
    task::{Context, Poll},
    time::Duration,
};

#[derive(Clone)]
pub(super) struct Reply {
    pub status: u16,
    pub body: String,
    pub location: Option<String>,
    pub header_delay: Duration,
    pub body_delay: Duration,
}
impl Default for Reply {
    fn default() -> Self {
        Self {
            status: 200,
            body: rotated().to_string(),
            location: None,
            header_delay: Duration::ZERO,
            body_delay: Duration::ZERO,
        }
    }
}
pub(super) fn rotated() -> serde_json::Value {
    serde_json::json!({"success":true,"data":{"accessToken":"rotated-synthetic-access",
        "refreshToken":"rotated-synthetic-refresh", "expiresAt":1790000000, "sgcookie":"rotated-cookie"}})
}

pub(super) struct Mock {
    pub endpoint: String,
    pub calls: Arc<Mutex<Vec<(HeaderMap, String)>>>,
    pub dropped_bodies: Arc<AtomicUsize>,
    task: Option<tokio::task::JoinHandle<()>>,
}
impl Mock {
    pub async fn new(reply: Reply) -> Self {
        let calls: Arc<Mutex<Vec<(HeaderMap, String)>>> = Arc::default();
        let captured = calls.clone();
        let dropped_bodies = Arc::new(AtomicUsize::new(0));
        let drops = dropped_bodies.clone();
        let app = Router::new().route(
            "/grant",
            post(move |headers: HeaderMap, body: String| {
                let reply = reply.clone();
                let calls = captured.clone();
                let drops = drops.clone();
                async move {
                    calls.lock().unwrap().push((headers, body));
                    tokio::time::sleep(reply.header_delay).await;
                    let mut builder = Response::builder()
                        .status(reply.status)
                        .header("content-type", "application/json");
                    if let Some(location) = &reply.location {
                        builder = builder.header("location", location);
                    }
                    let stream = TrackedBody {
                        delay: Box::pin(tokio::time::sleep(reply.body_delay)),
                        body: Some(reply.body.into_bytes()),
                        drops,
                    };
                    builder.body(Body::from_stream(stream)).unwrap()
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}/grant", listener.local_addr().unwrap());
        let task = Some(tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        }));
        Self {
            endpoint,
            calls,
            dropped_bodies,
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
    pub async fn wait_body_dropped(&self) {
        tokio::time::timeout(Duration::from_secs(3), async {
            while self.dropped_bodies.load(Ordering::SeqCst) == 0 {
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
impl Drop for Mock {
    fn drop(&mut self) {
        if let Some(task) = &self.task {
            task.abort();
        }
    }
}
struct TrackedBody {
    delay: Pin<Box<tokio::time::Sleep>>,
    body: Option<Vec<u8>>,
    drops: Arc<AtomicUsize>,
}
impl Stream for TrackedBody {
    type Item = Result<bytes::Bytes, Infallible>;
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        use std::future::Future;
        let this = self.get_mut();
        if this.delay.as_mut().poll(cx).is_pending() {
            return Poll::Pending;
        }
        Poll::Ready(this.body.take().map(|body| Ok(bytes::Bytes::from(body))))
    }
}
impl Drop for TrackedBody {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::SeqCst);
    }
}
pub(super) fn http() -> rquest::Client {
    let mut headers = rquest::header::HeaderMap::new();
    headers.insert("authorization", "Bearer must-not-leak".parse().unwrap());
    headers.insert("cookie", "private-cookie=must-not-leak".parse().unwrap());
    headers.insert("x-api-key", "must-not-leak".parse().unwrap());
    rquest::Client::builder()
        .no_proxy()
        .default_headers(headers)
        .build()
        .unwrap()
}
