//! Release an already received inference only after durable credential publication.
use super::*;
use crate::db::provider_credential_model_states::recovery::ModelHealthRecovery;
use axum::{extract::State, http::HeaderMap, response::Response, Router};
use std::sync::Mutex;
use tokio::sync::Semaphore;
mod cancellation;

#[derive(Clone)]
struct Reply {
    status: u16,
    arrived: Arc<Semaphore>,
    release: Arc<Semaphore>,
    closed: Arc<Semaphore>,
    calls: Arc<Mutex<Vec<HeaderMap>>>,
}
struct DelayedPeer {
    origin: String,
    reply: Reply,
    task: tokio::task::JoinHandle<()>,
}
impl DelayedPeer {
    async fn new(status: u16) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let reply = Reply {
            status,
            arrived: Arc::new(Semaphore::new(0)),
            release: Arc::new(Semaphore::new(0)),
            closed: Arc::new(Semaphore::new(0)),
            calls: Arc::new(Mutex::new(Vec::new())),
        };
        let app = Router::new().fallback(handler).with_state(reply.clone());
        let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        Self {
            origin,
            reply,
            task,
        }
    }
    fn client(&self) -> rquest::Client {
        // A missing origin seam must fail at loopback, never reach a real account.
        rquest::Client::builder()
            .no_proxy()
            .proxy(rquest::Proxy::all(&self.origin).unwrap())
            .timeout(Duration::from_secs(15))
            .build()
            .unwrap()
    }
}
impl Drop for DelayedPeer {
    fn drop(&mut self) {
        self.task.abort();
    }
}
async fn handler(State(reply): State<Reply>, request: Request<Body>) -> Response {
    assert_eq!(request.uri().path(), "/v1/chat/completions");
    let (parts, body) = request.into_parts();
    let raw = axum::body::to_bytes(body, 128 * 1024).await.unwrap();
    let body: Value = serde_json::from_slice(&raw).unwrap();
    assert_eq!(body["model"], "test-model");
    let first = {
        let mut calls = reply.calls.lock().unwrap();
        calls.push(parts.headers);
        calls.len() == 1
    };
    // A semaphore permit makes arrival/release deterministic without sleeps or polling.
    if first {
        reply.arrived.add_permits(1);
    }
    let status = if first { reply.status } else { 200 };
    let bytes = if status == 200 {
        native::reply("chat", false, false, true)
    } else {
        b"synthetic-inflight-auth-error".to_vec()
    };
    let body = if first && status != 200 {
        // Error probes finish on headers; hold those until rotation is published.
        reply.release.acquire().await.unwrap().forget();
        Body::from(bytes)
    } else if first {
        // Hold the success body: cancellation must release it without a permit.
        let closed = Closed(reply.closed);
        Body::from_stream(futures::stream::once(async move {
            let _closed = closed;
            reply.release.acquire().await.unwrap().forget();
            Ok::<_, std::io::Error>(bytes::Bytes::from(bytes))
        }))
    } else {
        Body::from(bytes)
    };
    Response::builder()
        .status(status)
        .header("content-type", "application/json")
        .body(body)
        .unwrap()
}

struct Closed(Arc<Semaphore>);
impl Drop for Closed {
    fn drop(&mut self) {
        self.0.add_permits(1);
    }
}

async fn rotated_inflight(status: u16) {
    let f = fixture().await;
    let pinned = f.runtime.route_config().snapshot();
    let old = pinned.select_credential_probe_target("account").unwrap();
    let original = request(&f);
    let peer = DelayedPeer::new(status).await;
    let (console, local) = state(&f, peer.client()).await;
    let mock = Mock::new(
        vec![(200, super::super::nous_target::subscription_reply())],
        0,
    )
    .await;
    let old_result =
        crate::protocol::nous_portal::test_support::origin(peer.origin.clone(), async {
            let rotate = async {
                tokio::time::timeout(Duration::from_secs(5), peer.reply.arrived.acquire())
                    .await
                    .unwrap()
                    .unwrap()
                    .forget();
                force_due(&f);
                Box::pin(refresh(&f, &mock, &mut Pending::new())).await;
                assert_eq!(
                    load(&f, &original).phase(),
                    crate::console::refresh_grants::GrantPhase::Published
                );
                // The old send is still owned by its caller; publication must not replay it.
                assert_eq!(peer.reply.calls.lock().unwrap().len(), 1);
                peer.reply.release.add_permits(1);
            };
            // Keep independent, large pipeline futures off the Windows test-thread stack.
            let (result, ()) = tokio::join!(Box::pin(call(&console.state, &old)), Box::pin(rotate));
            result
        })
        .await;
    assert_eq!(
        old_result.status,
        if status == 200 {
            ProviderPayloadProbeStatus::Passed
        } else {
            ProviderPayloadProbeStatus::Failed
        }
    );
    assert_eq!(
        old_result.health_recovery,
        Some(ModelHealthRecovery::StaleConfiguration)
    );
    assert!(
        local
            .list_model_states(db::CredentialModelStateFilters::default())
            .await
            .unwrap()
            .is_empty(),
        "old completion must not write health for the new binding"
    );
    let rows = local
        .list_audits(&db::RequestAuditFilters::default())
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0].status,
        if status == 200 { "completed" } else { "failed" }
    );
    assert_eq!(
        rows[0].total_tokens,
        if status == 200 { Some(5) } else { None }
    );
    let fresh = pinned.select_credential_probe_target("account").unwrap();
    assert_ne!(old.payload.api_key, fresh.payload.api_key);
    assert_eq!(
        fresh.payload.base_url,
        crate::protocol::nous_portal::InferenceTarget::Subscription.base_url()
    );
    let current = crate::protocol::nous_portal::test_support::origin(
        peer.origin.clone(),
        Box::pin(call(&console.state, &fresh)),
    )
    .await;
    assert_eq!(
        current.status,
        ProviderPayloadProbeStatus::Passed,
        "{current:?}"
    );
    assert_eq!(current.health_recovery, Some(ModelHealthRecovery::Applied));
    let calls = peer.reply.calls.lock().unwrap().clone();
    assert_eq!(calls.len(), 2);
    for (headers, target) in calls.iter().zip([&old, &fresh]) {
        assert_eq!(
            headers["authorization"],
            format!("Bearer {}", target.payload.api_key)
        );
    }
    assert_eq!(
        mock.count(),
        1,
        "inference must not consume another refresh token"
    );
    let rows = local
        .list_audits(&db::RequestAuditFilters::default())
        .await
        .unwrap();
    assert_eq!(rows.len(), 2);
    let saved = serde_json::to_string(&rows).unwrap();
    for secret in [
        &old.payload.api_key,
        &fresh.payload.api_key,
        &original.material.refresh,
    ] {
        assert!(!saved.contains(secret));
    }
    drop(console);
    local.close().await;
    mock.stop().await;
}

#[tokio::test]
async fn nous_probe_inflight_success_is_audited_without_healing_rotated_binding() {
    Box::pin(rotated_inflight(200)).await;
}
#[tokio::test]
async fn nous_probe_inflight_failure_is_audited_without_penalizing_rotated_binding() {
    Box::pin(rotated_inflight(401)).await;
}
