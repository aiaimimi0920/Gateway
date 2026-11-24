//! Opt-in Server checks against isolated PostgreSQL/Redis and a non-forwarding loopback supplier.
use super::{
    server_fixture::{build, close, run},
    *,
};
use crate::{
    provider_credential_probe_scheduler::start_provider_credential_probe_scheduler,
    provider_runtime::{
        probe_audit_test_support::{Hooks, Pause},
        probe_console_target_with_request, test_store, ConsoleProbeRequest,
        ProviderPayloadProbeReport, ProviderPayloadProbeStatus,
    },
};

const CREDENTIAL: &str = "scheduled";
const ANSWER: &str = "合成答案";

struct ServerFixture {
    scope: String,
    peer: peer::Peer,
}

impl ServerFixture {
    async fn new() -> Self {
        let reply = json!({"id":"synthetic-sensenova","model":UPSTREAM_MODEL,"choices":[{"index":0,
            "message":{"role":"assistant","content":ANSWER},"finish_reason":"stop"}],
            "usage":{"prompt_tokens":2,"completion_tokens":3,"total_tokens":5}});
        Self::with_peer(peer::Peer::raw(reply.to_string().into_bytes()).await)
    }

    fn with_peer(peer: peer::Peer) -> Self {
        Self {
            scope: format!("sensenova-server-{}", uuid::Uuid::new_v4().simple()),
            peer,
        }
    }

    async fn app(&self, instance: &str) -> Arc<AppState> {
        let document = json!({"providers":[{"id":self.scope,"preset":"sensenova-token-plan",
            "base_url":super::super::BASE,"explicit_credentials_only":true,"credentials":[{
                "id":CREDENTIAL,"api_key":KEY,"supported_models":[UPSTREAM_MODEL],"test_policy":{
                    "automaticEnabled":true,"intervalMinutes":1,"modelSelection":"selected",
                    "models":[UPSTREAM_MODEL],"cases":[{"id":"text","name":"Text","prompt":"only OK",
                        "expectedAnswer":ANSWER,"difficulty":1,"enabled":true}]}}]}],"model_routes":[]});
        build(&self.scope, instance, &self.peer, document).await
    }

    async fn in_peer<T>(&self, future: impl std::future::Future<Output = T>) -> T {
        super::super::test_support::origin(self.peer.origin.clone(), future).await
    }

    async fn probe(&self, state: &AppState) -> ProviderPayloadProbeReport {
        let target = state
            .route_config
            .snapshot()
            .select_credential_probe_target(CREDENTIAL)
            .unwrap();
        let request: ConsoleProbeRequest =
            serde_json::from_value(json!({"model":UPSTREAM_MODEL,"prompt":"only OK"})).unwrap();
        self.in_peer(probe_console_target_with_request(
            state,
            &target,
            Some(&request),
        ))
        .await
        .1
    }

    async fn audits(&self, state: &AppState) -> Vec<Audit> {
        let pg = state.pg_pool.as_ref().unwrap();
        let public_count: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM gateway_request_audits WHERE provider_account_id = $1",
        )
        .bind(&self.scope)
        .fetch_one(pg)
        .await
        .unwrap();
        assert_eq!(
            public_count, 0,
            "internal probes must not create public billing audits"
        );
        sqlx::query_as(
            "SELECT id, product, credential_id, model, status, upstream_status,
            prompt_tokens, completion_tokens, total_tokens, completed_at IS NOT NULL AS terminal
            FROM gateway_internal_model_probe_audits WHERE namespace = $1 AND provider_id = $1
            ORDER BY created_at, id LIMIT 3",
        )
        .bind(&self.scope)
        .fetch_all(pg)
        .await
        .unwrap()
    }

    async fn result(&self, state: &AppState) -> Option<Value> {
        let mut items = test_store::read_many(state, &self.scope, &[CREDENTIAL], None)
            .await
            .unwrap();
        let item = items.pop()?;
        assert!(items.is_empty());
        assert_eq!(item.status, ProviderPayloadProbeStatus::Passed);
        let result = serde_json::to_value(item).unwrap();
        assert_eq!(result["assessment"]["mode"], "automatic");
        assert_eq!(result["assessment"]["cases"][0]["correct"], true);
        Some(result)
    }
}

#[derive(Debug, PartialEq, sqlx::FromRow)]
struct Audit {
    id: String,
    product: String,
    credential_id: String,
    model: String,
    status: String,
    upstream_status: Option<i32>,
    prompt_tokens: Option<i64>,
    completion_tokens: Option<i64>,
    total_tokens: Option<i64>,
    terminal: bool,
}

#[test]
#[ignore = "Requires isolated PostgreSQL 001–007 and Redis; real 60-second worker ticks"]
fn sensenova_server_scheduler_audits_two_real_ticks_and_retains_state() {
    run(real_ticks);
}

async fn real_ticks() {
    let f = ServerFixture::new().await;
    let first = f.app("first").await;
    let second = f.app("second").await;
    for state in [&first, &second] {
        assert_eq!(
            state
                .route_config
                .snapshot()
                .scheduled_credential_probe_targets()
                .len(),
            1
        );
    }
    let one = Box::pin(f.in_peer(start_provider_credential_probe_scheduler(first.clone())));
    let two = Box::pin(f.in_peer(start_provider_credential_probe_scheduler(second.clone())));
    let started = std::time::Instant::now();
    let observe = async {
        let mut initial = None;
        loop {
            let calls = f.peer.calls.lock().unwrap().len();
            assert!(
                calls <= 2 && (calls < 2 || started.elapsed() >= Duration::from_secs(60)),
                "duplicate scheduled send"
            );
            if let Some(saved) = f.result(&first).await {
                let checked_at = saved["checkedAt"].as_str().unwrap();
                match &initial {
                    None => initial = Some(checked_at.to_owned()),
                    Some(earlier) if earlier != checked_at => {
                        first.shutdown.request("two-real-rounds-finished");
                        second.shutdown.request("two-real-rounds-finished");
                        break saved;
                    }
                    _ => {}
                }
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    };
    let (_, _, saved) = tokio::time::timeout(Duration::from_secs(135), async {
        tokio::join!(one, two, observe)
    })
    .await
    .expect("two real scheduler ticks must publish results");
    assert_wire(&f.peer, 2, "chat/completions");
    assert_eq!(f.result(&second).await.as_ref(), Some(&saved));
    let rows = f.audits(&first).await;
    assert_eq!(
        rows.len(),
        2,
        "each automatic generation needs one internal audit"
    );
    for row in &rows {
        assert_eq!(row.product, "sensenova_token_plan");
        assert_eq!(row.credential_id, CREDENTIAL);
        assert_eq!(row.model, UPSTREAM_MODEL);
        assert_eq!(row.status, "completed");
        assert!(row.terminal);
        assert_eq!(row.upstream_status, Some(200));
        assert_eq!(
            (row.prompt_tokens, row.completion_tokens, row.total_tokens),
            (Some(2), Some(3), Some(5))
        );
    }
    close(first).await;
    close(second).await;
    let rebuilt = f.app("restarted").await;
    assert_eq!(f.result(&rebuilt).await.as_ref(), Some(&saved));
    assert_eq!(f.audits(&rebuilt).await, rows);
    assert!(
        !test_store::claim(&rebuilt, &f.scope, CREDENTIAL, 1, None)
            .await
            .unwrap(),
        "rebuild must retain the live Redis slot"
    );
    close(rebuilt).await;
    f.peer.stop().await;
}

#[test]
#[ignore = "Requires isolated PostgreSQL/Redis; closes only the per-test pool"]
fn sensenova_server_probe_audit_unavailable_stops_before_send() {
    run(admission_failure);
}

async fn admission_failure() {
    let f = ServerFixture::new().await;
    let state = f.app("first").await;
    state.pg_pool.as_ref().unwrap().close().await;
    let result = f.probe(&state).await;
    assert_eq!(
        f.peer.calls.lock().unwrap().len(),
        0,
        "sent without internal audit admission"
    );
    assert_storage_failure(result, "no request was sent");
    close(state).await;
    f.peer.stop().await;
}

#[test]
#[ignore = "Requires isolated PostgreSQL 001–007 and Redis; closes only the per-test pool"]
fn sensenova_server_terminal_write_failure_does_not_publish_success_or_replay() {
    run(terminal_failure);
}

async fn terminal_failure() {
    let f = ServerFixture::new().await;
    let state = f.app("first").await;
    let observer = f.app("observer").await;
    let pause = Arc::new(Pause::default());
    let hooks = Hooks::pause_at_finish(pause.clone());
    let mut pending = Box::pin(hooks.within(f.probe(&state)));
    tokio::time::timeout(Duration::from_secs(5), async {
        tokio::select! {
            _ = pause.entered() => {},
            _ = &mut pending => panic!("probe finished without an audit finalizer"),
        }
    })
    .await
    .unwrap();
    let rows = f.audits(&observer).await;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].status, "running");
    assert!(!rows[0].terminal);
    assert_wire(&f.peer, 1, "chat/completions");
    state.pg_pool.as_ref().unwrap().close().await;
    pause.release();
    let result = tokio::time::timeout(Duration::from_secs(5), pending)
        .await
        .unwrap();
    assert_storage_failure(result, "request was not replayed");
    assert_wire(&f.peer, 1, "chat/completions");
    assert_eq!(f.audits(&observer).await, rows);
    close(state).await;
    close(observer).await;
    f.peer.stop().await;
}

#[test]
#[ignore = "Requires isolated PostgreSQL 001–007 and Redis"]
fn sensenova_server_failed_or_cancelled_probe_has_one_terminal_audit() {
    run(failed_and_cancelled);
}

async fn failed_and_cancelled() {
    let rejected = ServerFixture::with_peer(peer::Peer::new("chat", false, false, 401).await);
    let state = rejected.app("rejected").await;
    let report = rejected.probe(&state).await;
    assert_eq!(report.status, ProviderPayloadProbeStatus::Failed);
    assert!(report.answer.is_none());
    assert!(!report.message.contains("synthetic-private-error"));
    assert_wire(&rejected.peer, 1, "chat/completions");
    let rows = rejected.audits(&state).await;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].product, "sensenova_token_plan");
    assert_eq!(rows[0].status, "failed");
    assert!(rows[0].terminal);
    assert!(rows[0].total_tokens.is_none());
    close(state).await;
    rejected.peer.stop().await;

    let cancelled = ServerFixture::with_peer(peer::Peer::new("chat", false, true, 200).await);
    let state = cancelled.app("cancelled").await;
    let mut pending = Box::pin(cancelled.probe(&state));
    tokio::select! {
        _ = cancelled.peer.wait_calls() => {},
        _ = &mut pending => panic!("probe ended before cancellation"),
    }
    let admitted = cancelled.audits(&state).await;
    assert_eq!(admitted.len(), 1);
    assert_eq!(admitted[0].product, "sensenova_token_plan");
    assert_eq!(admitted[0].status, "running");
    drop(pending);
    cancelled.peer.wait_closed().await;
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let rows = cancelled.audits(&state).await;
            assert_eq!(rows.len(), 1);
            assert_eq!(rows[0].id, admitted[0].id);
            if rows[0].terminal {
                assert_eq!(rows[0].status, "cancelled");
                assert!(rows[0].total_tokens.is_none());
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    assert_wire(&cancelled.peer, 1, "chat/completions");
    close(state).await;
    cancelled.peer.stop().await;
}

fn assert_storage_failure(report: ProviderPayloadProbeReport, message: &str) {
    assert_eq!(report.status, ProviderPayloadProbeStatus::Failed);
    assert!(report.message.contains(message), "{report:?}");
    assert!(report.answer.is_none());
    assert_eq!(
        report.health_recovery,
        Some(db::provider_credential_model_states::recovery::ModelHealthRecovery::StorageFailed)
    );
}
