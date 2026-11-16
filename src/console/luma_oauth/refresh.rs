//! Explicit one-shot refresh: retain response before awaits, persist before interpreting, never resend uncertainty.
use super::{
    refresh_publication::Publication, save::SaveReceipt, LoginError, Session, SessionStatus,
};
use crate::{
    console::{
        refresh_grants::{GrantReceipt, GrantRequest, GrantReservation},
        RouteConfigRedisRevision,
    },
    error::GatewayError,
    protocol::luma_oauth::RefreshGrant,
    state::AppState,
};
use std::{sync::Arc, time::Duration};

#[derive(Clone)]
pub(super) struct RefreshTarget {
    pub revision: RouteConfigRedisRevision,
    pub request: GrantRequest,
}
pub(super) struct RefreshOperation {
    target: RefreshTarget,
    allow_send: bool,
}
impl Session {
    pub(super) fn begin_refresh(
        &self,
        resume: bool,
    ) -> Result<Option<RefreshOperation>, GatewayError> {
        let target = self
            .refresh
            .as_ref()
            .ok_or_else(|| super::login_error(LoginError::Unavailable))?;
        let mut inner = self.inner.lock();
        self.expire(&mut inner);
        if matches!(
            inner.status,
            SessionStatus::Importing | SessionStatus::Succeeded
        ) {
            return Ok(None);
        }
        if !(inner.status == SessionStatus::Ready
            || resume && inner.status == SessionStatus::SaveUnconfirmed)
        {
            return Err(super::login_error(LoginError::Unavailable));
        }
        let allow_send = inner.status == SessionStatus::Ready && !resume;
        inner.status = SessionStatus::Importing;
        inner.saved = None;
        Ok(Some(RefreshOperation {
            target: target.clone(),
            allow_send,
        }))
    }
}
impl RefreshOperation {
    pub(super) fn run(
        self,
        state: Arc<AppState>,
        session: Arc<Session>,
    ) -> impl std::future::Future<Output = ()> + Send + 'static {
        // Construct before polling, just as for login saves: rejected/dropped scheduling cannot remain busy.
        let mut guard = RefreshGuard {
            session: session.clone(),
            active: true,
        };
        async move {
            let result =
                tokio::time::timeout(Duration::from_secs(45), self.execute(&state, &session))
                    .await
                    .unwrap_or(Err("luma_refresh_unconfirmed"));
            let mut inner = session.inner.lock();
            let (status, id, revision, error) = match result {
                Ok(revision) => (
                    SessionStatus::Succeeded,
                    session.binding.credential.clone(),
                    Some(revision),
                    None,
                ),
                Err(code) => (SessionStatus::SaveUnconfirmed, None, None, Some(code)),
            };
            inner.status = status;
            inner.saved = Some(SaveReceipt::refreshed(id, revision, error));
            guard.disarm();
        }
    }

    async fn execute(&self, state: &AppState, session: &Session) -> Result<String, &'static str> {
        let runtime = state
            .route_config_runtime
            .as_ref()
            .ok_or("console_runtime_unavailable")?;
        // Repair an uncertain commit before inspecting the old grant; this only publishes stored evidence.
        {
            let _publication = state.lifecycle.health_control.publication.lock().await;
            runtime
                .recover_refresh_authority()
                .await
                .map_err(|e| e.code())?;
            runtime.repair_refresh_cache().await.map_err(|e| e.code())?;
        }
        let request = &self.target.request;
        let pending = session.inner.lock().refresh_pending.clone();
        let mut record = if let Some((record, _)) = &pending {
            record.clone()
        } else if let Some(record) = runtime
            .find_refresh_grant(request)
            .await
            .map_err(|e| e.code())?
        {
            record
        } else {
            if !self.allow_send || state.shutdown.is_requested() || state.lifecycle.is_draining() {
                return Err("luma_refresh_unconfirmed");
            }
            match runtime
                .reserve_refresh_grant(&self.target.revision, request)
                .await
                .map_err(|e| e.code())?
            {
                GrantReservation::Reserved(record) => {
                    let context = request
                        .material
                        .luma_oauth
                        .clone()
                        .ok_or("luma_refresh_rejected")?;
                    let grant = RefreshGrant::new(context, request.material.refresh.clone())
                        .map_err(|_| "luma_refresh_rejected")?;
                    let builder = rquest::Client::builder();
                    #[cfg(test)]
                    let builder = builder.no_proxy();
                    let http = builder.build().map_err(|_| "luma_refresh_unconfirmed")?;
                    let tokens = grant
                        .receive(&http)
                        .await
                        .map_err(|_| "luma_refresh_unconfirmed")?;
                    // Preserve the received successor across save failures and explicit disk-only resume.
                    session.inner.lock().refresh_pending =
                        Some((record.clone(), GrantReceipt::LumaOauth { tokens }));
                    record
                }
                GrantReservation::Existing(record) => record,
                _ => return Err("luma_refresh_authority_changed"),
            }
        };
        if !record.matches_request(request) {
            return Err("luma_refresh_authority_changed");
        }
        let pending = session.inner.lock().refresh_pending.clone();
        if let Some((_, receipt)) = pending {
            record = runtime
                .store_refresh_grant_receipt(&record, &receipt)
                .await
                .map_err(|e| e.code())?;
            session.inner.lock().refresh_pending = None;
        }
        if record.result().is_none() {
            let Some(GrantReceipt::LumaOauth { tokens }) = record.receipt_for(request) else {
                return Err("luma_refresh_unconfirmed"); // An old Sending with no receipt never authorizes another request.
            };
            let result = request
                .result_with_luma(tokens)
                .map_err(|_| "luma_refresh_response_rejected")?;
            record = runtime
                .store_refresh_grant_result(&record, &result)
                .await
                .map_err(|e| e.code())?;
        }
        let _publication = state.lifecycle.health_control.publication.lock().await;
        runtime
            .commit_refresh_ticket(&Publication(record.clone()))
            .await
            .map_err(|e| e.code())?;
        runtime
            .mark_refresh_grant_published(&record)
            .await
            .map_err(|e| e.code())?;
        Ok(state.route_config.snapshot().revision().id().into())
    }
}

struct RefreshGuard {
    session: Arc<Session>,
    active: bool,
}
impl RefreshGuard {
    fn disarm(&mut self) {
        self.active = false;
    }
}
impl Drop for RefreshGuard {
    fn drop(&mut self) {
        if self.active {
            let mut inner = self.session.inner.lock();
            inner.status = SessionStatus::SaveUnconfirmed;
            inner.saved = Some(SaveReceipt::refreshed(
                None,
                None,
                Some("luma_refresh_unconfirmed"),
            ));
        }
    }
}
