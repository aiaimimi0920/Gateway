//! Probe outcome does not silently change account-wide or newer model health.
use super::fixture::*;
use std::sync::atomic::Ordering;

async fn entered(fixture: &Fixture) {
    tokio::time::timeout(
        std::time::Duration::from_secs(10),
        fixture.gate.entered.notified(),
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn missing_initial_health_row_does_not_override_concurrent_failure() {
    let fixture = Fixture::new(true).await;
    let probe = fixture.probe();
    let failure = async {
        entered(&fixture).await;
        fixture.cooling().await;
        fixture.gate.release.add_permits(1);
    };
    let (report, ()) = tokio::join!(probe, failure);
    assert_eq!(report["status"], "passed");
    assert_eq!(report["healthRecovery"], "stale_health");
    let rows = fixture
        .local
        .list_model_states(Default::default())
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].failure_count, 1);
    assert_eq!(rows[0].status, "cooling");
    fixture.finish().await;
}

#[tokio::test]
async fn generated_reply_preserves_model_hard_block() {
    let fixture = Fixture::new(true).await;
    let mut failure = fixture.failure();
    failure.upstream_status = Some(401);
    failure.classification = neuro_gateway::provider_failure::classify_provider_failure(
        Some(401),
        None,
        Some("unauthorized"),
    );
    fixture.local.model_failure(failure).await.unwrap();
    fixture.gate.release.add_permits(1);
    let report = fixture.probe().await;
    assert_eq!(report["status"], "passed");
    assert_eq!(report["healthRecovery"], "protected");
    let rows = fixture
        .local
        .list_model_states(Default::default())
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].status, "blocked");
    fixture.finish().await;
}

#[tokio::test]
async fn failed_generation_records_only_the_exact_routing_identity() {
    let fixture = Fixture::new(true).await;
    fixture.gate.status.store(429, Ordering::SeqCst);
    fixture.gate.release.add_permits(1);
    let report = fixture.probe().await;
    assert_eq!(report["status"], "failed");
    assert_eq!(report["healthRecovery"], "failure_recorded");
    let mut candidates = [fixture.candidate.clone()];
    fixture
        .local
        .apply_model_states(&mut candidates)
        .await
        .unwrap();
    assert!(candidates[0].cooldown_until.is_some());
    assert_eq!(candidates[0].failure_count, 1);
    fixture.finish().await;
}

#[tokio::test]
async fn late_probe_failure_cannot_downgrade_a_new_hard_block() {
    let fixture = Fixture::new(true).await;
    fixture.cooling().await;
    fixture.gate.status.store(429, Ordering::SeqCst);
    let probe = fixture.probe();
    let blocked = async {
        entered(&fixture).await;
        let mut failure = fixture.failure();
        failure.upstream_status = Some(401);
        failure.classification = neuro_gateway::provider_failure::classify_provider_failure(
            Some(401),
            None,
            Some("unauthorized"),
        );
        fixture.local.model_failure(failure).await.unwrap();
        fixture.gate.release.add_permits(1);
    };
    let (report, ()) = tokio::join!(probe, blocked);
    assert_eq!(report["status"], "failed");
    assert_eq!(report["healthRecovery"], "stale_health");
    let rows = fixture
        .local
        .list_model_states(Default::default())
        .await
        .unwrap();
    assert_eq!(rows[0].status, "blocked");
    fixture.finish().await;
}

#[tokio::test]
async fn storage_failure_does_not_turn_a_real_reply_into_confirmed_recovery() {
    let fixture = Fixture::new(true).await;
    fixture.cooling().await;
    let probe = fixture.probe();
    let close = async {
        entered(&fixture).await;
        fixture.local.close().await;
        fixture.gate.release.add_permits(1);
    };
    let (report, ()) = tokio::join!(probe, close);
    assert_eq!(report["status"], "passed");
    assert_eq!(report["healthRecovery"], "storage_failed");
    fixture.finish().await;
}

#[tokio::test]
async fn test_plan_keeps_recovery_per_case_and_does_not_claim_a_plan_wide_reset() {
    let fixture = Fixture::new(true).await;
    fixture.cooling().await;
    fixture.gate.release.add_permits(1);
    let response = send_console_json(
        &fixture.console.state,
        axum::http::Method::POST,
        "/v1/internal/gateway/console/providers/pool/probe",
        serde_json::json!({"testPlan":{"modelSelection":"selected", "models":["public-model"],
            "cases":[{"id":"one", "name":"One", "prompt":"OK", "difficulty":1, "enabled":true}]}}),
        Some(&fixture.grant),
        None,
    )
    .await;
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    let body = parse_json(response).await;
    let result = &body["result"]["results"][0];
    assert_eq!(result["status"], "passed");
    assert!(result.get("healthRecovery").is_none());
    assert_eq!(
        result["assessment"]["cases"][0]["healthRecovery"],
        "applied"
    );
    fixture.finish().await;
}
