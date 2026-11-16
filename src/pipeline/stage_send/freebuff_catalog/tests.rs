//! Real public/management TCP, synthetic FreeBuff only, real Local auth/reservations/claim store.
use serde_json::{json, Value};
use std::time::Duration;
mod boundaries;
mod cancellation;
mod host;
mod peer;
mod protocol_json;
mod protocol_sse;
mod public_sse;
mod purchase_cycle;
mod run_journal;
mod send_fences;
#[path = "../../../../tests/console_contract_support/mod.rs"]
mod support;
mod ui;
mod ui_sse;
use host::Host;
fn request() -> Value {
    json!({"model":"open","messages":[{"role":"system","content":"caller policy"},{"role":"user","content":"private prompt"}],"max_tokens":16})
}
async fn claims(host: &Host) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM local_freebuff_session_claims")
        .fetch_one(&host.state.local_runtime.as_ref().unwrap().pool)
        .await
        .unwrap()
}
async fn settled(host: &Host, status: &str, balance: i64) {
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let rows = host
                .state
                .local_runtime
                .as_ref()
                .unwrap()
                .list_audits(&crate::db::RequestAuditFilters::default())
                .await
                .unwrap();
            if let Some(row) = rows.first().filter(|r| r.status != "running") {
                assert_eq!(row.status, status);
                assert_eq!(row.access_key_id.as_deref(), Some(host.key_id.as_str()));
                assert_eq!(host.balance().await, balance);
                assert_eq!(
                    host.state
                        .concurrency_registry
                        .get_or_create("pool")
                        .snapshot()
                        .active_count,
                    0
                );
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
}
#[tokio::test]
async fn freebuff_public_chat_waits_before_permit_then_uses_original_signed_run() {
    let peer = peer::Peer::new().await;
    let host = Box::pin(Host::new(&peer)).await;
    host.seed_device(&peer).await;
    let grant = host.grant().await;
    let task = host.public(request());
    let view = host.pending(&grant, &task).await;
    assert_eq!(view["quote"]["walletSpend"], 2.0);
    assert_eq!(view["quote"]["modelKey"], "open");
    assert_eq!(
        host.state
            .concurrency_registry
            .get_or_create("pool")
            .snapshot()
            .active_count,
        0
    );
    assert_eq!(peer.calls("/api/v1/freebuff/session/admission"), 0);
    assert_eq!(claims(&host).await, 0);
    assert_eq!(host.decision(&grant, &view, "approve").await, 200);
    let (status, body) = task.await.unwrap();
    assert_eq!(status, 200, "{body}");
    assert_eq!(body, peer::completion());
    assert_eq!(peer.calls("/api/v1/freebuff/session/admission"), 1);
    assert_eq!(peer.calls("/api/v1/chat/completions"), 1);
    assert_eq!(
        claims(&host).await,
        1,
        "FINISH never silently releases or reuses the paid session"
    );
    let finish = peer.finishes();
    assert_eq!(finish.len(), 1);
    assert_eq!(finish[0]["status"], "completed");
    assert_eq!(finish[0]["totalSteps"], 1);
    assert_eq!(finish[0]["totalCredits"], 13);
    assert!(host.list(&grant).await.as_array().unwrap().is_empty());
    settled(&host, "completed", 9).await;
    host.close().await;
    peer.server.close().await;
}
#[tokio::test]
async fn freebuff_public_chat_cancel_and_caller_drop_never_purchase_or_hold_permit() {
    for mode in ["cancel", "drop"] {
        let peer = peer::Peer::new().await;
        let host = Box::pin(Host::new(&peer)).await;
        host.seed_device(&peer).await;
        let grant = host.grant().await;
        let task = host.public(request());
        let view = host.pending(&grant, &task).await;
        if mode == "cancel" {
            assert_eq!(host.decision(&grant, &view, "cancel").await, 200);
            let (status, _) = task.await.unwrap();
            assert_eq!(status, 409);
            settled(&host, "failed", 10).await;
        } else {
            task.abort();
            let _ = task.await;
            settled(&host, "cancelled", 10).await;
        }
        assert_eq!(peer.calls("/api/v1/freebuff/session/admission"), 0);
        assert_eq!(claims(&host).await, 0);
        assert!(host.list(&grant).await.as_array().unwrap().is_empty());
        assert_eq!(host.decision(&grant, &view, "approve").await, 409);
        host.close().await;
        peer.server.close().await;
    }
}
#[tokio::test]
async fn freebuff_public_chat_rejects_unsupported_intent_before_quote_or_purchase() {
    let peer = peer::Peer::new().await;
    let host = Box::pin(Host::new(&peer)).await;
    host.seed_device(&peer).await;
    let before = peer.hits.lock().unwrap().len();
    for (key, value) in [
        ("stream", json!("true")),
        ("n", json!(2)),
        ("codebuff_metadata", json!({"runId":"forged"})),
        ("unknown", json!(true)),
        ("parallel_tool_calls", json!("yes")),
        ("seed", json!(0.5)),
        ("stop", json!(["text", 1])),
        ("tool_choice", json!("required")),
    ] {
        let mut body = request();
        body[key] = value;
        let (status, error) = host.public(body).await.unwrap();
        assert_eq!(status, 400, "{key}: {error}");
    }
    for message in [
        json!({"role":"assistant","content":"text","audio":{"id":"hidden"}}),
        json!({"role":"user","content":[{"type":"text","text":"hello","audio":{}}]}),
    ] {
        let mut body = request();
        body["messages"][1] = message;
        let (status, error) = host.public(body).await.unwrap();
        assert_eq!(status, 400, "{error}");
    }
    assert_eq!(peer.hits.lock().unwrap().len(), before);
    assert_eq!(claims(&host).await, 0);
    assert_eq!(host.balance().await, 10);
    host.close().await;
    peer.server.close().await;
}
#[tokio::test]
async fn freebuff_public_chat_invalid_or_unknown_completion_never_retries_or_rebuys() {
    for mode in ["invalid", "unknown"] {
        let peer = peer::Peer::new().await;
        *peer.mode.lock().unwrap() = mode.into();
        let host = Box::pin(Host::new(&peer)).await;
        host.seed_device(&peer).await;
        let grant = host.grant().await;
        let task = host.public(request());
        let view = host.pending(&grant, &task).await;
        assert_eq!(host.decision(&grant, &view, "approve").await, 200);
        let (status, error) = task.await.unwrap();
        assert_eq!(status, 502, "{mode}: {error}");
        assert_eq!(peer.calls("/api/v1/freebuff/session/admission"), 1);
        assert_eq!(peer.calls("/api/v1/chat/completions"), 1);
        let finishes = peer.finishes();
        if mode == "invalid" {
            assert_eq!(finishes.len(), 1);
            assert_eq!(finishes[0]["status"], "failed");
        } else {
            assert!(finishes.is_empty(), "unknown is not zero");
        }
        assert_eq!(claims(&host).await, 1);
        settled(&host, "failed", 10).await;
        let calls = peer.hits.lock().unwrap().len();
        let (status, _) = host.public(request()).await.unwrap();
        assert_eq!(status, 409);
        assert_eq!(peer.calls("/api/v1/freebuff/session/admission"), 1);
        assert_eq!(peer.calls("/api/v1/chat/completions"), 1);
        assert!(host.list(&grant).await.as_array().unwrap().is_empty());
        assert_eq!(
            peer.hits.lock().unwrap().len(),
            calls + 1,
            "only signed catalog refresh; no foreign claim heartbeat or quote"
        );
        host.close().await;
        peer.server.close().await;
    }
}
