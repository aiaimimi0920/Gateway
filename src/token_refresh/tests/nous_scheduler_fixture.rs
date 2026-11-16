//! Token endpoint witness is explicitly loopback-scoped; no synthetic token can reach a real host.
use super::*;
use axum::{
    body::Bytes,
    extract::State,
    http::{HeaderMap, StatusCode},
    routing::post,
    Router,
};
use serde_json::json;
use std::collections::VecDeque;

struct Shared {
    replies: parking_lot::Mutex<VecDeque<(u16, String)>>,
    requests: parking_lot::Mutex<Vec<(HeaderMap, String)>>,
    called: tokio::sync::Notify,
    delay_ms: u64,
}
pub(super) struct Mock {
    pub origin: String,
    shared: Arc<Shared>,
    task: Option<tokio::task::JoinHandle<()>>,
}
impl Mock {
    pub async fn new(replies: Vec<(u16, String)>, delay_ms: u64) -> Self {
        let shared = Arc::new(Shared {
            replies: parking_lot::Mutex::new(replies.into()),
            requests: Default::default(),
            called: Default::default(),
            delay_ms,
        });
        let router = Router::new()
            .route("/api/oauth/token", post(handle))
            .with_state(shared.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let task = Some(tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap()
        }));
        Self {
            origin,
            shared,
            task,
        }
    }
    pub fn count(&self) -> usize {
        self.shared.requests.lock().len()
    }
    pub fn requests(&self) -> Vec<(HeaderMap, String)> {
        self.shared.requests.lock().clone()
    }
    pub async fn wait_called(&self) {
        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                let notified = self.shared.called.notified();
                if self.count() > 0 {
                    break;
                }
                notified.await;
            }
        })
        .await
        .unwrap();
    }
    pub async fn stop(mut self) {
        if let Some(task) = self.task.take() {
            task.abort();
            assert!(task.await.unwrap_err().is_cancelled());
        }
    }
}
impl Drop for Mock {
    fn drop(&mut self) {
        if let Some(task) = &self.task {
            task.abort();
        }
    }
}
async fn handle(
    State(shared): State<Arc<Shared>>,
    headers: HeaderMap,
    body: Bytes,
) -> (StatusCode, String) {
    shared
        .requests
        .lock()
        .push((headers, String::from_utf8(body.to_vec()).unwrap()));
    shared.called.notify_one();
    let (status, body) = shared
        .replies
        .lock()
        .pop_front()
        .expect("unexpected repeated grant request");
    tokio::time::sleep(Duration::from_millis(shared.delay_ms)).await;
    (StatusCode::from_u16(status).unwrap(), body)
}
pub(super) fn reply() -> String {
    json!({"access_token":super::nous_fixture::access("synthetic-subject",4102444900),
        "refresh_token":"nous-new-refresh","token_type":"Bearer"})
    .to_string()
}
pub(super) fn force_due(f: &LocalFixture) {
    f.runtime
        .route_config()
        .snapshot()
        .oauth_refresh_target("oauth", "account")
        .unwrap()
        .1
        .refresh_config
        .as_ref()
        .unwrap()
        .force_due();
}
pub(super) async fn refresh(f: &LocalFixture, mock: &Mock, pending: &mut Pending) {
    crate::protocol::nous_portal::TEST_ORIGIN
        .scope(mock.origin.clone(), f.refresh(pending))
        .await;
}
