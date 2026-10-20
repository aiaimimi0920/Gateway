//! Run the same expected-row contract against Local and the guarded opt-in PG fixture.
use super::health_store::HealthStore;
use neuro_gateway::{
    db::{
        self,
        provider_credential_model_states::recovery::{
            self, ModelHealthRecovery as Outcome, ModelHealthSnapshot,
        },
    },
    routing::candidate::RouteCandidate,
};

impl HealthStore<'_> {
    async fn snapshot(&self, input: db::RecordCredentialModelSuccessInput) -> ModelHealthSnapshot {
        match self {
            Self::Local(local) => local.model_health_snapshot(input).await,
            Self::Pg(pool) => recovery::snapshot(pool, input).await,
        }
        .unwrap()
    }

    async fn recover(
        &self,
        expected: &ModelHealthSnapshot,
        failure: Option<db::RecordCredentialModelFailureInput>,
    ) -> Outcome {
        match self {
            Self::Local(local) => local.apply_model_health_snapshot(expected, failure).await,
            Self::Pg(pool) => recovery::apply(pool, expected, failure).await,
        }
        .unwrap()
    }
}

pub async fn contract(store: HealthStore<'_>, candidate: RouteCandidate) {
    for credential_id in [None, Some("db-credential".to_string())] {
        let identity = db::RecordCredentialModelSuccessInput {
            provider_account_id: candidate.provider_account_id.clone(),
            provider_credential_id: credential_id,
            provider_credential_ref: Some("route-reference".into()),
            protocol_profile: Some(candidate.protocol_profile.clone()),
            model: "recovery-contract".into(),
        };
        let failure = db::RecordCredentialModelFailureInput {
            provider_account_id: identity.provider_account_id.clone(),
            provider_credential_id: identity.provider_credential_id.clone(),
            provider_credential_ref: identity.provider_credential_ref.clone(),
            protocol_profile: identity.protocol_profile.clone(),
            model: identity.model.clone(),
            upstream_status: Some(429),
            upstream_retry_after_ms: None,
            error_message: Some("rate limit".into()),
            classification: neuro_gateway::provider_failure::classify_provider_failure(
                Some(429),
                None,
                Some("rate limit"),
            ),
        };
        // Initially absent -> concurrent failure: do not create an active replacement.
        let absent = store.snapshot(identity.clone()).await;
        store.failure(failure.clone()).await;
        assert_eq!(store.recover(&absent, None).await, Outcome::StaleHealth);

        // ABA-shaped transition: failure -> success -> failure returns to count one.
        let old = store.snapshot(identity.clone()).await;
        store.success(identity.clone()).await;
        let current = store.failure(failure.clone()).await;
        assert_eq!(current.failure_count, 1);
        assert_eq!(store.recover(&old, None).await, Outcome::StaleHealth);

        let expected = store.snapshot(identity.clone()).await;
        let (first, second) = tokio::join!(
            store.recover(&expected, None),
            store.recover(&expected, None)
        );
        assert!(matches!(
            (first, second),
            (Outcome::Applied, Outcome::StaleHealth) | (Outcome::StaleHealth, Outcome::Applied)
        ));

        // A late failed probe also cannot downgrade a newer hard block.
        let old = store.snapshot(identity.clone()).await;
        let mut blocked = failure.clone();
        blocked.upstream_status = Some(401);
        blocked.classification = neuro_gateway::provider_failure::classify_provider_failure(
            Some(401),
            None,
            Some("unauthorized"),
        );
        store.failure(blocked).await;
        assert_eq!(
            store.recover(&old, Some(failure)).await,
            Outcome::StaleHealth
        );
        let protected = store.snapshot(identity).await;
        assert_eq!(store.recover(&protected, None).await, Outcome::Protected);
    }
}

#[tokio::test]
async fn local_expected_health_cas_protects_failure_absence_aba_and_parallel_recovery() {
    use super::fixture::{candidate, TestState, Upstream};
    use futures::FutureExt;
    let upstream = Upstream::start(axum::http::StatusCode::OK, false, "{}").await;
    let fixture = TestState::with_local_limits().await;
    let result = std::panic::AssertUnwindSafe(contract(
        HealthStore::Local(fixture.state.local_runtime.as_ref().unwrap()),
        candidate(&upstream, "health-cas"),
    ))
    .catch_unwind()
    .await;
    upstream.finish().await;
    fixture.finish().await;
    if let Err(panic) = result {
        std::panic::resume_unwind(panic);
    }
}
