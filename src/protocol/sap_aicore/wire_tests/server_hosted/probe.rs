//! Hosted probe facts are cross-instance readable, separate from caller audits and billing.
use super::*;
use crate::provider_runtime::{
    probe_audit_test_support::{Hooks, Pause},
    probe_console_target_with_request, ConsoleProbeRequest, ProviderPayloadProbeReport,
    ProviderPayloadProbeStatus,
};
mod storage;

const SCHEMA: &str =
    include_str!("../../../../../deploy/postgres/initdb/012-gateway-sap-probe-evidence.sql");

async fn app(scope: &str, instance: &str, peer: &peer::Peer) -> Arc<AppState> {
    let state = build(scope, instance, peer).await;
    // Hosted health rows require a real account FK; console credential refs are separate.
    let (provider, _) = provision(
        &state,
        scope,
        json!({"sap_api_version":"2024-10-21"}),
        json!({"api_key":KEY}),
    )
    .await;
    state.route_config.replace_document(serde_json::from_value(json!({
        "providers":[{"id":provider,"preset":"sap-aicore-chat","base_url":test_support::BASE,
            "explicit_credentials_only":true,"headers":{"AI-Resource-Group":"group-a"},
            "extra_body":{"sap_api_version":"2024-10-21"},
            "supported_models":[UPSTREAM_MODEL],
            "credentials":[{"id":"exact","api_key":KEY,"supported_models":[UPSTREAM_MODEL]}]}],
        "model_routes":[]
    })).unwrap()).unwrap();
    state
}

async fn probe(peer: &peer::Peer, state: &AppState) -> ProviderPayloadProbeReport {
    let target = state
        .route_config
        .snapshot()
        .select_credential_probe_target("exact")
        .unwrap();
    let request: ConsoleProbeRequest = serde_json::from_value(json!({
        "model":UPSTREAM_MODEL,"prompt":"only OK"
    }))
    .unwrap();
    test_support::origin(
        peer.origin.clone(),
        probe_console_target_with_request(state, &target, Some(&request)),
    )
    .await
    .1
}

#[derive(Debug, PartialEq, sqlx::FromRow)]
struct Audit {
    status: String,
    upstream_status: Option<i32>,
    total_tokens: Option<i64>,
    sap_evidence: Option<Value>,
}

async fn rows(state: &AppState, scope: &str) -> Vec<Audit> {
    let pg = state.pg_pool.as_ref().unwrap();
    let public: i64 = sqlx::query_scalar("SELECT count(*) FROM gateway_request_audits
        WHERE provider_account_id IN (SELECT provider_id FROM gateway_internal_model_probe_audits WHERE namespace=$1)")
        .bind(scope).fetch_one(pg).await.unwrap();
    assert_eq!(public, 0, "probe cannot create public/debit audits");
    sqlx::query_as(
        "SELECT status,upstream_status,total_tokens,sap_evidence
        FROM gateway_internal_model_probe_audits WHERE namespace=$1
        AND credential_id='exact' AND model=$2 AND product='sap_aicore' ORDER BY created_at,id",
    )
    .bind(scope)
    .bind(UPSTREAM_MODEL)
    .fetch_all(pg)
    .await
    .unwrap()
}

async fn terminal(state: &AppState, scope: &str) -> Audit {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let mut found = rows(state, scope).await;
            if found.len() == 1 && found[0].status != "running" {
                return found.remove(0);
            }
            assert!(found.len() <= 1);
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap()
}

#[test]
#[ignore = "Requires dedicated PostgreSQL/Redis; retains isolated probe evidence"]
fn sap_aicore_server_probe_phase_health_and_cross_instance_evidence() {
    run_hosted(classified);
}

async fn classified() {
    for (oauth, status) in [
        (true, 401),
        (true, 429),
        (false, 401),
        (false, 429),
        (false, 503),
        (false, 200),
    ] {
        let peer = peer::Peer::new("chat", false, false, if oauth { 200 } else { status }).await;
        *peer.retry_after.lock().unwrap() = Some("120".into());
        if oauth {
            *peer.token.lock().unwrap() = (status, b"private-oauth-error".to_vec());
        }
        let scope = format!("sap-probe-{}", uuid::Uuid::new_v4().simple());
        let writer = app(&scope, "writer", &peer).await;
        let reader = build(&scope, "reader", &peer).await;
        for _ in 0..2 {
            sqlx::raw_sql(SCHEMA)
                .execute(writer.pg_pool.as_ref().unwrap())
                .await
                .unwrap();
        }
        let report = probe(&peer, &writer).await;
        assert_eq!(
            report.status == ProviderPayloadProbeStatus::Passed,
            status == 200
        );
        let audit = terminal(&reader, &scope).await;
        assert_eq!(
            audit.status,
            if status == 200 { "completed" } else { "failed" }
        );
        assert_eq!(
            audit.upstream_status,
            if oauth { None } else { Some(i32::from(status)) }
        );
        assert_eq!(audit.total_tokens, (status == 200).then_some(5));
        let evidence = audit.sap_evidence.unwrap();
        assert_eq!(evidence["attempts"], u32::from(!oauth));
        if status != 200 {
            assert_eq!(
                evidence["failure"]["phase"],
                if oauth { "oauth" } else { "inference" }
            );
            assert_eq!(evidence["failure"]["status"], status);
            assert_eq!(evidence["failure"]["retryAfterMs"], 120_000);
        }
        assert!(!evidence.to_string().contains("private-oauth-error"));
        let health: Vec<(String, i32, Option<i32>)> = sqlx::query_as(
            "SELECT provider_credential_ref,failure_count,last_upstream_status
             FROM gateway_provider_credential_model_states WHERE provider_account_id=$1",
        )
        .bind(&writer.route_config.snapshot().document().providers[0].id)
        .fetch_all(reader.pg_pool.as_ref().unwrap())
        .await
        .unwrap();
        if !oauth && matches!(status, 429 | 503) {
            assert_eq!(health, vec![("exact".into(), 1, Some(i32::from(status)))]);
        } else if status == 200 {
            assert_eq!(health, vec![("exact".into(), 0, None)]);
        } else {
            assert!(health.is_empty());
        }
        use crate::db::provider_credential_model_states::recovery::ModelHealthRecovery;
        assert_eq!(
            report.health_recovery,
            Some(if status == 200 {
                ModelHealthRecovery::Applied
            } else if !oauth && matches!(status, 429 | 503) {
                ModelHealthRecovery::FailureRecorded
            } else {
                ModelHealthRecovery::FailureNotPenalized
            })
        );
        assert_eq!(peer.calls.lock().unwrap().len(), usize::from(!oauth));
        finish(writer, reader, peer).await;
    }
}

#[test]
#[ignore = "Requires dedicated PostgreSQL/Redis; creates no public request"]
fn sap_aicore_server_probe_unclaimed_create_cancellation_has_closed_zero_send_evidence() {
    run_hosted(cancel_creation);
}

async fn cancel_creation() {
    let peer = peer::Peer::new("chat", false, false, 200).await;
    let scope = format!("sap-probe-cancel-{}", uuid::Uuid::new_v4().simple());
    let writer = app(&scope, "writer", &peer).await;
    let reader = build(&scope, "reader", &peer).await;
    sqlx::raw_sql(SCHEMA)
        .execute(writer.pg_pool.as_ref().unwrap())
        .await
        .unwrap();
    let pause = Arc::new(Pause::default());
    let hooks = Hooks {
        created: Some(pause.clone()),
        ..Hooks::default()
    };
    let mut pending = Box::pin(hooks.within(probe(&peer, &writer)));
    tokio::time::timeout(Duration::from_secs(5), async {
        tokio::select! { _ = pause.entered() => {}, _ = &mut pending => panic!("creation barrier missed") }
    }).await.unwrap();
    let running = rows(&reader, &scope).await;
    assert_eq!(running.len(), 1);
    assert!(
        running[0].sap_evidence.is_none(),
        "running is unknown, not proof of non-delivery"
    );
    drop(pending);
    pause.release();
    let audit = terminal(&reader, &scope).await;
    assert_eq!(audit.status, "cancelled");
    assert_eq!(audit.sap_evidence.unwrap()["attempts"], 0);
    assert!(peer.oauth.lock().unwrap().is_empty() && peer.calls.lock().unwrap().is_empty());
    finish(writer, reader, peer).await;
}
