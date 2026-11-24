//! Verify Server probe persistence and rejection without public caller/usage attribution.
use super::*;
use crate::provider_runtime::probe_audit_test_support::{self, Hooks, Pause};
use crate::provider_runtime::{probe_console_target_with_request, ConsoleProbeRequest};
use crate::provider_runtime::{ProviderPayloadProbeReport, ProviderPayloadProbeStatus};

#[derive(Debug, PartialEq, sqlx::FromRow)]
pub(super) struct Audit {
    id: String,
    pub credential_id: String,
    pub model: String,
    status: String,
    prompt_tokens: Option<i64>,
    completion_tokens: Option<i64>,
    total_tokens: Option<i64>,
    terminal: bool,
}
impl Audit {
    pub(super) fn assert_terminal(&self, status: &str, usage: Option<(i64, i64, i64)>) {
        assert_eq!(self.status, status);
        assert!(self.terminal);
        assert_eq!(
            (
                self.prompt_tokens,
                self.completion_tokens,
                self.total_tokens
            ),
            usage.map_or((None, None, None), |(one, two, total)| (
                Some(one),
                Some(two),
                Some(total)
            ))
        );
    }
}

pub(super) async fn rows(state: &AppState, namespace: &str, provider: &str) -> Vec<Audit> {
    sqlx::query_as(
        "SELECT id, credential_id, model, status, prompt_tokens, completion_tokens,
        total_tokens, completed_at IS NOT NULL AS terminal FROM gateway_internal_model_probe_audits
        WHERE namespace = $1 AND product = 'commandcode' AND provider_id = $2
        ORDER BY created_at, id LIMIT 3",
    )
    .bind(namespace)
    .bind(provider)
    .fetch_all(state.pg_pool.as_ref().unwrap())
    .await
    .unwrap()
}

pub(super) async fn assert_no_public_audits(state: &AppState, model: &str) {
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM gateway_request_audits WHERE requested_model = $1",
    )
    .bind(model)
    .fetch_one(state.pg_pool.as_ref().unwrap())
    .await
    .unwrap();
    assert_eq!(
        count, 0,
        "internal probes must not create public request audits"
    );
}

async fn fixture(peer: &peer::Peer) -> (String, Arc<AppState>) {
    let namespace = format!("commandcode-probe-{}", uuid::Uuid::new_v4().simple());
    let root = std::path::PathBuf::from(std::env::var("COMMANDCODE_TEST_EVIDENCE_ROOT").unwrap());
    assert!(root.starts_with("C:/Users/Public/nas_home/AI/GameEditor/linshi"));
    let state = server::state_document(&root.join(&namespace), &namespace, peer, json!({
        "providers":[{"id":namespace,"preset":super::super::PRESET,"base_url":super::super::BASE,
            "explicit_credentials_only":true,"credentials":[
                {"id":"probe","api_key":KEY,"supported_models":[namespace]}
            ]}],"model_routes":[]
    })).await;
    (namespace, state)
}

async fn probe(peer: &peer::Peer, state: &AppState, model: &str) -> ProviderPayloadProbeReport {
    let target = state
        .route_config
        .snapshot()
        .select_credential_probe_target("probe")
        .unwrap();
    let request: ConsoleProbeRequest =
        serde_json::from_value(json!({"model":model,"prompt":"only OK"})).unwrap();
    super::super::test_support::origin(
        peer.origin.clone(),
        probe_console_target_with_request(state, &target, Some(&request)),
    )
    .await
    .1
}

#[test]
#[ignore = "Requires isolated PostgreSQL and Redis with internal model-probe audit migration"]
fn commandcode_server_probe_audit_failure_and_cancellation_are_terminal() {
    server_scheduler::run(terminal_cases);
}

async fn terminal_cases() {
    let peer = peer::Peer::new("chat", false, false, 401).await;
    let (namespace, state) = fixture(&peer).await;
    assert_eq!(
        probe(&peer, &state, &namespace).await.status,
        ProviderPayloadProbeStatus::Failed
    );
    let audits = rows(&state, &namespace, &namespace).await;
    assert_eq!(audits.len(), 1);
    audits[0].assert_terminal("failed", None);
    assert_eq!(peer.calls.lock().unwrap().len(), 1);
    assert_no_public_audits(&state, &namespace).await;
    server_scheduler::close(state).await;
    peer.stop().await;

    let peer = peer::Peer::new("chat", false, true, 200).await;
    let (namespace, state) = fixture(&peer).await;
    let mut in_flight = Box::pin(probe(&peer, &state, &namespace));
    tokio::select! {
        _ = peer.wait_calls() => {},
        _ = &mut in_flight => panic!("probe finished before cancellation"),
    }
    let audits = rows(&state, &namespace, &namespace).await;
    assert_eq!(
        audits.len(),
        1,
        "admission persisted before the upstream send"
    );
    assert_eq!(audits[0].status, "running");
    drop(in_flight);
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let result = rows(&state, &namespace, &namespace).await;
            assert_eq!(result.len(), 1);
            assert_eq!(result[0].id, audits[0].id);
            if result[0].terminal {
                result[0].assert_terminal("cancelled", None);
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(peer.calls.lock().unwrap().len(), 1);
    assert_no_public_audits(&state, &namespace).await;
    server_scheduler::close(state).await;
    peer.stop().await;
}

#[test]
#[ignore = "Requires isolated PostgreSQL and Redis; closes only its own pool"]
fn commandcode_server_probe_audit_unavailable_stops_before_send() {
    server_scheduler::run(audit_unavailable);
}

async fn audit_unavailable() {
    let peer = peer::Peer::new("chat", false, false, 200).await;
    let (namespace, state) = fixture(&peer).await;
    state.pg_pool.as_ref().unwrap().close().await;
    assert_storage_failure(
        probe(&peer, &state, &namespace).await,
        "no request was sent",
    );
    assert!(peer.calls.lock().unwrap().is_empty());
    server_scheduler::close(state).await;
    peer.stop().await;
}

#[test]
#[ignore = "Requires isolated gateway_fixture_unmigrated database with migrations 001–006 only"]
fn commandcode_server_probe_audit_missing_migration_stops_before_send() {
    server_scheduler::run(missing_migration);
}

async fn missing_migration() {
    let peer = peer::Peer::new("chat", false, false, 200).await;
    let (namespace, state) = fixture(&peer).await;
    let (database, has_public_audits, has_internal_audits): (String, bool, bool) = sqlx::query_as(
        "SELECT current_database(), to_regclass('gateway_request_audits') IS NOT NULL,
                to_regclass('gateway_internal_model_probe_audits') IS NOT NULL",
    )
    .fetch_one(state.pg_pool.as_ref().unwrap())
    .await
    .unwrap();
    assert_eq!(database, "gateway_fixture_unmigrated");
    assert!(has_public_audits && !has_internal_audits);
    let result = probe(&peer, &state, &namespace).await;
    assert_storage_failure(result, "no request was sent");
    assert!(peer.calls.lock().unwrap().is_empty());
    assert_no_public_audits(&state, &namespace).await;
    server_scheduler::close(state).await;
    peer.stop().await;
}

#[test]
#[ignore = "Requires isolated migrated PostgreSQL and Redis; closes only its own pool"]
fn commandcode_server_probe_audit_terminal_write_failure_does_not_report_success_or_replay() {
    server_scheduler::run(terminal_write_failure);
}

async fn terminal_write_failure() {
    let peer = peer::Peer::new("chat", false, false, 200).await;
    let (namespace, state) = fixture(&peer).await;
    let pause = Arc::new(Pause::default());
    let hooks = Hooks::pause_at_finish(pause.clone());
    let mut in_flight = Box::pin(hooks.within(probe(&peer, &state, &namespace)));
    tokio::time::timeout(Duration::from_secs(5), async {
        tokio::select! {
            _ = pause.entered() => {},
            _ = &mut in_flight => panic!("probe finished before the terminal write barrier"),
        }
    })
    .await
    .unwrap();
    let audits = rows(&state, &namespace, &namespace).await;
    assert_eq!(audits.len(), 1);
    assert_eq!(audits[0].status, "running");
    assert_eq!(peer.calls.lock().unwrap().len(), 1);
    assert_no_public_audits(&state, &namespace).await;

    // Bound the negative control to this AppState; another observer still has a healthy pool.
    let observer = server::state_document(
        &std::path::PathBuf::from(std::env::var("COMMANDCODE_TEST_EVIDENCE_ROOT").unwrap())
            .join(&namespace)
            .join("observer"),
        &namespace,
        &peer,
        json!({"providers":[],"model_routes":[]}),
    )
    .await;
    state.pg_pool.as_ref().unwrap().close().await;
    pause.release();
    let result = tokio::time::timeout(Duration::from_secs(5), in_flight)
        .await
        .unwrap();
    assert_storage_failure(result, "request was not replayed");
    assert_eq!(peer.calls.lock().unwrap().len(), 1);
    assert_eq!(rows(&observer, &namespace, &namespace).await, audits);
    server_scheduler::close(observer).await;
    server_scheduler::close(state).await;
    peer.stop().await;
}

#[test]
#[ignore = "Requires isolated PostgreSQL and Redis"]
fn commandcode_server_public_audit_requires_caller_identity() {
    server_scheduler::run(public_audit_needs_identity);
}

async fn public_audit_needs_identity() {
    let peer = peer::Peer::new("chat", false, false, 200).await;
    let (namespace, state) = fixture(&peer).await;
    let mut input = probe_audit_test_support::input();
    input.project_id = state.config.default_project_id.clone();
    input.requested_model = Some(namespace.clone());
    input.provider_account_id = None;
    let error = db::create_request_audit(state.pg_pool.as_ref().unwrap(), input)
        .await
        .unwrap_err();
    assert_eq!(
        error.message,
        "request audit requires api_key_id, user_credential_id, or access_key_id"
    );
    assert_no_public_audits(&state, &namespace).await;
    server_scheduler::close(state).await;
    peer.stop().await;
}

fn assert_storage_failure(report: ProviderPayloadProbeReport, message: &str) {
    assert_eq!(report.status, ProviderPayloadProbeStatus::Failed);
    assert!(report.message.contains(message));
    assert!(report.answer.is_none());
    assert_eq!(report.health_recovery, Some(crate::db::provider_credential_model_states::recovery::ModelHealthRecovery::StorageFailed));
}
