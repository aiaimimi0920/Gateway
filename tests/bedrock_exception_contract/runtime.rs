//! Isolated production Local bootstrap and two real random-loopback HTTP servers.
use super::support;
use axum::{extract::State, http::HeaderMap, routing::post, Json, Router};
use futures::StreamExt;
use neuro_gateway::{
    access_balance::AccessBalanceStore, config::GatewayStorageMode, db::RequestAuditFilters,
    http::router::build_router, state::AppState,
};
use serde_json::{json, Value};
use std::{
    net::SocketAddr,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use tokio::{sync::oneshot, task::JoinHandle, time::timeout};

pub(super) const DEADLINE: Duration = Duration::from_secs(10);

struct Server {
    origin: String,
    stop: Option<oneshot::Sender<()>>,
    task: JoinHandle<()>,
}
impl Server {
    async fn start(app: Router) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let (stop, stopped) = oneshot::channel();
        let task = tokio::spawn(async move {
            axum::serve(
                listener,
                app.into_make_service_with_connect_info::<SocketAddr>(),
            )
            .with_graceful_shutdown(async {
                let _ = stopped.await;
            })
            .await
            .unwrap();
        });
        Self {
            origin,
            stop: Some(stop),
            task,
        }
    }
    async fn close(mut self) {
        let _ = self.stop.take().unwrap().send(());
        timeout(DEADLINE, &mut self.task).await.unwrap().unwrap();
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}

#[derive(Clone)]
struct Reply {
    body: Arc<Mutex<(Vec<u8>, Option<Vec<u8>>)>>,
    calls: Arc<AtomicUsize>,
    release: Arc<tokio::sync::Notify>,
}
async fn upstream(
    State(reply): State<Reply>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> impl axum::response::IntoResponse {
    assert_eq!(headers["authorization"], "Bearer synthetic-only");
    assert!(headers.get("cookie").is_none());
    assert_eq!(body["messages"][0]["content"][0]["text"], "hello");
    reply.calls.fetch_add(1, Ordering::SeqCst);
    let (first, tail) = reply.body.lock().unwrap().clone();
    let stream =
        futures::stream::once(async move { Ok::<_, std::io::Error>(bytes::Bytes::from(first)) })
            .chain(futures::stream::once(async move {
                if let Some(tail) = tail {
                    reply.release.notified().await;
                    Ok(bytes::Bytes::from(tail))
                } else {
                    Ok(bytes::Bytes::new())
                }
            }));
    (
        [("content-type", "application/vnd.amazon.eventstream")],
        axum::body::Body::from_stream(stream),
    )
}

pub(super) struct Fixture {
    pub state: Arc<AppState>,
    pub token: String,
    pub key_id: String,
    pub calls: Arc<AtomicUsize>,
    reply: Arc<Mutex<(Vec<u8>, Option<Vec<u8>>)>>,
    release: Arc<tokio::sync::Notify>,
    client: rquest::Client,
    local: Server,
    upstream: Server,
}
impl Fixture {
    pub async fn start() -> Self {
        let reply = Reply {
            body: Default::default(),
            calls: Default::default(),
            release: Default::default(),
        };
        let upstream = Server::start(
            Router::new()
                .route("/model/fixture-model/converse-stream", post(upstream))
                .route("/model/fixture-model/converse", post(upstream))
                .with_state(reply.clone()),
        )
        .await;
        let root = std::env::temp_dir().join(format!(
            "gateway-bedrock-exception-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let mut config = support::test_config();
        config.storage_mode = GatewayStorageMode::Local;
        config.console.state_dir = root.join("state");
        config.console.routes_file = root.join("routes.yaml");
        let provider = |id: &str| {
            json!({"id":id,"adapter":"bedrock_converse_compatible","protocol_profile":"aws_bedrock",
            "base_url":upstream.origin,"supported_models":["fixture-model"],
            "credentials":[{"id":format!("{id}-account"),"api_key":"synthetic-only","supported_models":["fixture-model"]}]})
        };
        let routes = json!({"providers":[provider("primary"),provider("fallback")],
            "model_routes":[{"pattern":"fixture-model","provider_ids":["primary","fallback"]}],"aliases":{}});
        std::fs::write(
            &config.console.routes_file,
            serde_yaml::to_string(&routes).unwrap(),
        )
        .unwrap();
        let state = Box::pin(neuro_gateway::runtime::build_app_state(config))
            .await
            .unwrap();
        assert!(
            state.local_runtime.is_some()
                && state.redis_pool.is_closed()
                && state.pg_pool.is_none()
        );
        let local = Server::start(build_router(state.clone())).await;
        let client = rquest::Client::builder()
            .no_proxy()
            .timeout(DEADLINE)
            .build()
            .unwrap();
        let created:Value=client.post(format!("{}/v1/internal/gateway/access/keys",local.origin))
            .header("x-management-token",support::MANAGEMENT_TOKEN)
            .json(&json!({"ownerType":"user","ownerId":"fixture","resolvedProjectId":"local","resolvedTenantId":"local",
                "keyKind":"normal","publicKeyPrefix":"sk-gw","displayName":"bedrock-exceptions","metadata":{"models":["fixture-model"]}}))
            .send().await.unwrap().error_for_status().unwrap().json().await.unwrap();
        let key_id = created["id"].as_str().unwrap().to_owned();
        let token = created["token"].as_str().unwrap().to_owned();
        client
            .post(format!(
                "{}/v1/internal/gateway/access/keys/{key_id}/balances/adjust",
                local.origin
            ))
            .header("x-management-token", support::MANAGEMENT_TOKEN)
            .json(&json!({"balanceMode":"message_prepaid","totalMessages":2,"remainingMessages":2}))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap();
        Self {
            state,
            token,
            key_id,
            calls: reply.calls,
            reply: reply.body,
            release: reply.release,
            client,
            local,
            upstream,
        }
    }
    pub fn reply(&self, body: Vec<u8>) {
        *self.reply.lock().unwrap() = (body, None);
    }
    pub fn held(&self, first: Vec<u8>, tail: Vec<u8>) {
        *self.reply.lock().unwrap() = (first, Some(tail));
    }
    pub fn release(&self) {
        self.release.notify_one();
    }
    pub async fn open(&self, path: &str, body: Value) -> rquest::Response {
        self.client
            .post(format!("{}{path}", self.local.origin))
            .bearer_auth(&self.token)
            .header("cookie", "caller-must-not-forward")
            .json(&body)
            .send()
            .await
            .unwrap()
    }
    pub async fn settled(&self, count: usize) {
        let local = self.state.local_runtime.as_ref().unwrap();
        timeout(DEADLINE, async {
            loop {
                let rows = local
                    .list_audits(&RequestAuditFilters::default())
                    .await
                    .unwrap();
                if rows.len() == count && rows.iter().all(|r| r.status != "running") {
                    assert!(rows.iter().all(|r| r.status == "failed"));
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        let balance = AccessBalanceStore::from_state(&self.state)
            .unwrap()
            .get(&self.key_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            balance.remaining_messages,
            Some(2),
            "failed inference must refund Local pre-deduction"
        );
        assert_eq!(
            self.calls.load(Ordering::SeqCst),
            count,
            "no retry or fallback after a response started"
        );
        for candidate in self
            .state
            .route_config
            .snapshot()
            .resolve_candidates(Some("fixture-model"))
        {
            let controller = self
                .state
                .concurrency_registry
                .get_or_create(&candidate.provider_account_id);
            assert_eq!(controller.snapshot().active_count, 0);
            assert_eq!(controller.snapshot().consecutive_successes, 0);
        }
    }
    pub async fn close(self) {
        self.local.close().await;
        self.upstream.close().await;
        self.state.shutdown.request("bedrock_exception_test");
        timeout(DEADLINE, self.state.local_runtime.as_ref().unwrap().close())
            .await
            .unwrap();
    }
}
