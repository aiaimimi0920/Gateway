//! Narrow grant operations share the authority backend; Workers gain no general mutation API.
use super::*;
use crate::console::refresh_grants::{
    GrantReceipt, GrantRecord, GrantRequest, GrantReservation, GrantResult,
};

pub(super) fn unsupported() -> RouteConfigRedisStoreError {
    RouteConfigRedisStoreError::from_persistence(PersistenceError::recovery_required(
        "Backend cannot retain OAuth refresh grant consumption evidence",
    ))
}

impl RouteConfigRuntime {
    pub(crate) async fn find_refresh_grant(
        &self,
        request: &GrantRequest,
    ) -> Result<Option<GrantRecord>, RouteConfigRuntimeError> {
        self.refresh_coordinator
            .redis
            .find_refresh_grant(request)
            .await
            .map_err(RouteConfigRuntimeError::from_redis)
    }

    pub(crate) async fn reserve_refresh_grant(
        &self,
        revision: &RouteConfigRedisRevision,
        request: &GrantRequest,
    ) -> Result<GrantReservation, RouteConfigRuntimeError> {
        self.refresh_coordinator
            .redis
            .reserve_refresh_grant(revision, request)
            .await
            .map_err(RouteConfigRuntimeError::from_redis)
    }

    pub(crate) async fn store_refresh_grant_receipt(
        &self,
        record: &GrantRecord,
        receipt: &GrantReceipt,
    ) -> Result<GrantRecord, RouteConfigRuntimeError> {
        self.refresh_coordinator
            .redis
            .store_refresh_grant_receipt(record, receipt)
            .await
            .map_err(RouteConfigRuntimeError::from_redis)
    }

    pub(crate) async fn store_refresh_grant_result(
        &self,
        record: &GrantRecord,
        result: &GrantResult,
    ) -> Result<GrantRecord, RouteConfigRuntimeError> {
        self.refresh_coordinator
            .redis
            .store_refresh_grant_result(record, result)
            .await
            .map_err(RouteConfigRuntimeError::from_redis)
    }

    pub(crate) async fn mark_refresh_grant_published(
        &self,
        record: &GrantRecord,
    ) -> Result<GrantRecord, RouteConfigRuntimeError> {
        let snapshot = self.route_config.snapshot();
        let revision =
            RouteConfigRedisRevision::new(snapshot.revision().clone(), snapshot.document().clone())
                .map_err(RouteConfigRuntimeError::from_redis)?;
        self.refresh_coordinator
            .redis
            .mark_refresh_grant_published(&revision, record)
            .await
            .map_err(RouteConfigRuntimeError::from_redis)
    }
}
