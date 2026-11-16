//! Old/custom backends cannot accidentally authorize rollback via a default success.
use super::*;

struct Unsupported(fixture::Backend);
#[async_trait]
impl RouteConfigRedisBackend for Unsupported {
    async fn store_revision(
        &self,
        r: &RouteConfigRedisRevision,
    ) -> Result<(), RouteConfigRedisStoreError> {
        self.0.store_revision(r).await
    }
    async fn store_prepared_transaction(
        &self,
        r: &TransactionRecord,
    ) -> Result<(), RouteConfigRedisStoreError> {
        self.0.store_prepared_transaction(r).await
    }
    async fn activate_revision(
        &self,
        e: Option<&str>,
        r: &RouteConfigRedisRevision,
        p: &TransactionRecord,
        a: &TransactionRecord,
    ) -> Result<RouteConfigRedisActivationOutcome, RouteConfigRedisStoreError> {
        self.0.activate_revision(e, r, p, a).await
    }
    async fn load_active_revision(
        &self,
    ) -> Result<Option<RouteConfigRedisRevision>, RouteConfigRedisStoreError> {
        self.0.load_active_revision().await
    }
}

#[tokio::test]
async fn unsupported_backend_never_rolls_back_before_reporting_failure() {
    let harness = Harness::new(Stage::Activate);
    cancel_after_acceptance(&harness).await;
    let replaced = std::fs::read(&harness.routes).unwrap();
    let runtime = RouteConfigRuntime::with_backend(
        harness.runtime.route_config().clone(),
        harness.persistence.clone(),
        Arc::new(Unsupported(harness.backend.clone())),
        true,
    );
    let error = runtime.recover_startup().await.unwrap_err();
    assert_eq!(error.code(), "console_recovery_required");
    assert_eq!(
        harness.journal.load_transactions().unwrap()[0].phase(),
        TransactionPhase::YamlReplaced
    );
    assert_eq!(std::fs::read(&harness.routes).unwrap(), replaced);
    harness.runtime.recover_startup().await.unwrap();
    assert!(matches!(
        harness.backend.release_and_join().await,
        Reply::Activated(RouteConfigRedisActivationOutcome::PreparedTransactionMismatch)
    ));
}
