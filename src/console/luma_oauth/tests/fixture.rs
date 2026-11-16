use super::{account, support};
use crate::console::luma_oauth::{Binding, Session, SessionStatus};
use axum::{routing::post, Json, Router};
use serde_json::{json, Value};
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};
pub(super) const PATH: &str = "/v1/internal/gateway/console/luma-auth-sessions";

pub(super) struct Peer {
    pub endpoint: String,
    calls: Arc<AtomicUsize>,
    task: Option<tokio::task::JoinHandle<()>>,
}
impl Peer {
    pub async fn new(delay: Duration) -> Self {
        Self::with_reply(
            delay,
            json!({"access_token":"synthetic-luma-access","token_type":"Bearer","expires_in":3600}),
        )
        .await
    }
    pub async fn with_reply(delay: Duration, reply: Value) -> Self {
        let calls = Arc::new(AtomicUsize::new(0));
        let observed = calls.clone();
        let app = Router::new().route(
            "/token",
            post(move || {
                let calls = observed.clone();
                let reply = reply.clone();
                async move {
                    calls.fetch_add(1, Ordering::SeqCst);
                    tokio::time::sleep(delay).await;
                    Json(reply)
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}/token", listener.local_addr().unwrap());
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        Self {
            endpoint,
            calls,
            task: Some(task),
        }
    }
    pub fn count(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
    pub async fn wait_call(&self) {
        tokio::time::timeout(Duration::from_secs(3), async {
            while self.count() == 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
    }
    pub async fn stop(mut self) {
        let task = self.task.take().unwrap();
        task.abort();
        let _ = task.await;
    }
}
impl Drop for Peer {
    fn drop(&mut self) {
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}
pub(super) fn redirect() -> String {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    format!(
        "http://127.0.0.1:{}/callback",
        listener.local_addr().unwrap().port()
    )
}
pub(super) fn fixture(redirect: &str, existing: bool) -> support::ConsoleStateFixture {
    let mut p = account::provider();
    p["credentials"][0]["luma_oauth"]["redirect_uri"] = json!(redirect);
    p["credentials"][0]["account_name"] = json!("preserved account name");
    if !existing {
        p["credentials"] = json!([]);
    }
    support::ConsoleStateFixture::new(account::document(p), true)
}
pub(super) fn binding(f: &support::ConsoleStateFixture, redirect: &str) -> Binding {
    Binding::new(
        "owner",
        "luma-mcp",
        f.state.route_config.snapshot().revision().id(),
        "registered-client",
        redirect,
    )
    .unwrap()
}
pub(super) fn callback(session: &Session, state_override: Option<&str>) -> String {
    let view = session.view();
    let authorize = url::Url::parse(view.authorization_url.as_deref().unwrap()).unwrap();
    let state = authorize
        .query_pairs()
        .find(|(key, _)| key == "state")
        .unwrap()
        .1
        .into_owned();
    let mut callback = url::Url::parse(&view.redirect_uri).unwrap();
    callback
        .query_pairs_mut()
        .append_pair("state", state_override.unwrap_or(&state))
        .append_pair("code", "synthetic-code");
    callback.into()
}
pub(super) fn callback_view(view: &Value) -> String {
    let authorize = url::Url::parse(view["authorizationUrl"].as_str().unwrap()).unwrap();
    let state = authorize
        .query_pairs()
        .find(|(key, _)| key == "state")
        .unwrap()
        .1
        .into_owned();
    let mut callback = url::Url::parse(view["redirectUri"].as_str().unwrap()).unwrap();
    callback
        .query_pairs_mut()
        .append_pair("state", &state)
        .append_pair("code", "synthetic-code");
    callback.into()
}
pub(super) async fn await_status(session: &Session, expected: SessionStatus) {
    tokio::time::timeout(Duration::from_secs(3), async {
        while session.view().status != expected {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
}
pub(super) fn assert_port_free(redirect: &str) {
    let port = url::Url::parse(redirect).unwrap().port().unwrap();
    assert!(std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port)).is_ok());
}
