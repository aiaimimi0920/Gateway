//! Generation success and exact routing recovery are separate, race-safe outcomes.
#[path = "console_model_recovery/fixture.rs"]
mod fixture;
#[path = "console_model_recovery/outcomes.rs"]
mod outcomes;
use fixture::*;
use neuro_gateway::db;

#[tokio::test]
async fn generated_reply_recovers_exact_pooled_and_default_route_identity() {
    for pooled in [true, false] {
        let fixture = Fixture::new(pooled).await;
        fixture.cooling().await;
        let mut other = fixture.candidate.clone();
        other.protocol_profile = "another-profile".into();
        fixture
            .local
            .model_failure(failure_for(&other))
            .await
            .unwrap();
        fixture.gate.release.add_permits(1);
        let report = fixture.probe().await;
        assert_eq!(report["status"], "passed");
        assert_eq!(report["healthRecovery"], "applied");
        let mut candidates = [fixture.candidate.clone(), other];
        fixture
            .local
            .apply_model_states(&mut candidates)
            .await
            .unwrap();
        assert!(
            candidates[0].cooldown_until.is_none(),
            "probe must clear the row normal routing reads"
        );
        assert!(candidates[1].cooldown_until.is_some());
        let rows = fixture
            .local
            .list_model_states(db::CredentialModelStateFilters::default())
            .await
            .unwrap();
        assert_eq!(rows.len(), 2, "do not create a profile-less probe-only row");
        fixture.finish().await;
    }
}

#[tokio::test]
async fn late_success_preserves_failure_written_during_generation() {
    let fixture = Fixture::new(true).await;
    fixture.cooling().await;
    let probe = fixture.probe();
    let update = async {
        tokio::time::timeout(
            std::time::Duration::from_secs(10),
            fixture.gate.entered.notified(),
        )
        .await
        .unwrap();
        fixture.cooling().await;
        fixture.gate.release.add_permits(1);
    };
    let (report, ()) = tokio::join!(probe, update);
    assert_eq!(report["status"], "passed");
    assert_eq!(report["healthRecovery"], "stale_health");
    let rows = fixture
        .local
        .list_model_states(Default::default())
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].failure_count, 2);
    assert_eq!(rows[0].status, "cooling");
    fixture.finish().await;
}

#[tokio::test]
async fn configuration_change_during_probe_does_not_recover_old_binding() {
    let fixture = Fixture::new(true).await;
    fixture.cooling().await;
    let probe = fixture.probe();
    let change = async {
        tokio::time::timeout(
            std::time::Duration::from_secs(10),
            fixture.gate.entered.notified(),
        )
        .await
        .unwrap();
        let mut document = fixture.document.clone();
        document.providers[0].label = Some("changed while generating".into());
        fixture
            .console
            .state
            .route_config
            .replace_document(document)
            .unwrap();
        fixture.gate.release.add_permits(1);
    };
    let (report, ()) = tokio::join!(probe, change);
    assert_eq!(report["status"], "passed");
    assert_eq!(report["healthRecovery"], "stale_configuration");
    let rows = fixture
        .local
        .list_model_states(Default::default())
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].status, "cooling");
    fixture.finish().await;
}
