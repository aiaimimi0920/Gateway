//! Pooled authority backend forwarding, separate from runtime roles and coordinator ownership.
use super::*;
use crate::console;

#[async_trait]
impl RouteConfigRedisBackend for PooledRouteConfigRedisBackend {
    async fn load_refresh_grant(
        &self,
        locator: &console::refresh_grants::GrantLocator,
    ) -> Result<Option<console::refresh_grants::GrantRecord>, RouteConfigRedisStoreError> {
        self.store.load_refresh_grant(&self.pool, locator).await
    }

    async fn repair_refresh_grant_cache(
        &self,
        revision: &RouteConfigRedisRevision,
        record: &console::refresh_grants::GrantRecord,
        transaction: &TransactionRecord,
    ) -> Result<(), RouteConfigRedisStoreError> {
        self.store
            .repair_refresh_grant_cache(&self.pool, revision, record, transaction)
            .await
    }

    async fn reserve_refresh_grant(
        &self,
        revision: &RouteConfigRedisRevision,
        request: &console::refresh_grants::GrantRequest,
    ) -> Result<console::refresh_grants::GrantReservation, RouteConfigRedisStoreError> {
        self.store
            .reserve_refresh_grant(&self.pool, revision, request)
            .await
    }

    async fn store_refresh_grant_receipt(
        &self,
        record: &console::refresh_grants::GrantRecord,
        receipt: &console::refresh_grants::GrantReceipt,
    ) -> Result<console::refresh_grants::GrantRecord, RouteConfigRedisStoreError> {
        self.store
            .store_refresh_grant_receipt(&self.pool, record, receipt)
            .await
    }

    async fn store_refresh_grant_result(
        &self,
        record: &console::refresh_grants::GrantRecord,
        result: &console::refresh_grants::GrantResult,
    ) -> Result<console::refresh_grants::GrantRecord, RouteConfigRedisStoreError> {
        self.store
            .store_refresh_grant_result(&self.pool, record, result)
            .await
    }

    async fn mark_refresh_grant_published(
        &self,
        revision: &RouteConfigRedisRevision,
        record: &console::refresh_grants::GrantRecord,
    ) -> Result<console::refresh_grants::GrantRecord, RouteConfigRedisStoreError> {
        self.store
            .mark_refresh_grant_published(&self.pool, revision, record)
            .await
    }

    async fn settle_transaction_locked(
        &self,
        _guard: &console::WriterLockGuard,
        record: &TransactionRecord,
    ) -> Result<RouteConfigRedisSettlement, RouteConfigRedisStoreError> {
        self.store.settle_transaction(&self.pool, record).await
    }

    async fn store_revision(
        &self,
        revision: &RouteConfigRedisRevision,
    ) -> Result<(), RouteConfigRedisStoreError> {
        self.store.store_revision(&self.pool, revision).await
    }

    async fn store_prepared_transaction(
        &self,
        record: &TransactionRecord,
    ) -> Result<(), RouteConfigRedisStoreError> {
        self.store
            .store_prepared_transaction(&self.pool, record)
            .await
    }

    async fn activate_revision(
        &self,
        expected_active_revision: Option<&str>,
        revision: &RouteConfigRedisRevision,
        prepared_record: &TransactionRecord,
        activated_record: &TransactionRecord,
    ) -> Result<RouteConfigRedisActivationOutcome, RouteConfigRedisStoreError> {
        self.store
            .activate_revision(
                &self.pool,
                expected_active_revision,
                revision,
                prepared_record,
                activated_record,
            )
            .await
    }

    async fn load_active_revision(
        &self,
    ) -> Result<Option<RouteConfigRedisRevision>, RouteConfigRedisStoreError> {
        self.store.load_active_revision(&self.pool).await
    }
}
