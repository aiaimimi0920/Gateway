//! The kernel guard spans send and its live observation; dropping never erases the durable claim.
use super::*;
use crate::protocol::freebuff::catalog::{BusinessReply, CatalogRun, OwnedActiveSession};
use std::time::Duration;
pub(crate) struct ActiveAdmission {
    _owner: SessionRecovery,
    active: OwnedActiveSession,
}
impl PendingAdmission<'_> {
    pub(crate) fn claim(&self) -> &SessionClaim {
        &self.owner.claim
    }
    pub(crate) async fn send(self, budget: Duration) -> Result<ActiveAdmission, GatewayError> {
        let reply = self.request.send(budget).await.map_err(business_error)?;
        self.owner.confirm(reply)
    }
}
impl SessionRecovery {
    pub(crate) fn claim(&self) -> &SessionClaim {
        &self.claim
    }
    pub(crate) fn model_key(&self) -> &str {
        &self.model_key
    }
    pub(crate) async fn heartbeat(
        self,
        requests: &RecoveryRequests<'_>,
        budget: Duration,
    ) -> Result<ActiveAdmission, GatewayError> {
        let reply = requests
            .heartbeat(&self.claim, budget)
            .await
            .map_err(business_error)?;
        self.confirm(reply)
    }
    fn confirm(self, reply: BusinessReply) -> Result<ActiveAdmission, GatewayError> {
        let active = OwnedActiveSession::parse(
            reply,
            &self.claim,
            &self.model_key,
            time::OffsetDateTime::now_utc(),
        )
        .map_err(business_error)?;
        Ok(ActiveAdmission {
            _owner: self,
            active,
        })
    }
}
impl ActiveAdmission {
    pub(crate) fn active(&self) -> &OwnedActiveSession {
        &self.active
    }
    /// The exclusive borrow keeps the original kernel guard alive through START and response body.
    /// This does not renew the observation or provide fee confirmation, agent harness or FINISH.
    pub(crate) async fn start_catalog_run<'a>(
        &'a mut self,
        requests: &'a BusinessRequests<'a>,
        budget: Duration,
    ) -> Result<CatalogRun<'a>, BusinessError> {
        CatalogRun::start_recorded(
            requests,
            &self.active,
            budget,
            Box::new(self._owner.run_store()),
        )
        .await
    }
}
