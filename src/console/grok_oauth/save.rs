//! Accepted saves outlive their HTTP waiter; only the normal commit result establishes success.
use super::{
    persistence::{self, PreparedSave},
    Binding, LoginError, Session, SessionStatus,
};
use crate::{error::GatewayError, protocol::grok_oauth::GrantReceipt, state::AppState};
use std::{sync::Arc, time::Duration};
use tokio::time::Instant;

pub(super) struct SaveReceipt {
    pub credential_id: Option<String>,
    pub revision: Option<String>,
    pub error_code: Option<&'static str>,
    finished: Instant,
}
impl SaveReceipt {
    pub(super) fn retain(&self) -> bool {
        self.finished.elapsed() < Duration::from_secs(300)
    }
}
pub(super) struct SaveOperation {
    prepared: PreparedSave,
    receipt: GrantReceipt,
}
impl Session {
    pub(super) fn begin_save(
        &self,
        state: &AppState,
        binding: &Binding,
    ) -> Result<Option<SaveOperation>, GatewayError> {
        let mut inner = self.inner.lock();
        self.expire(&mut inner);
        if self.binding != *binding {
            return Err(super::login_error(LoginError::Unavailable));
        }
        if matches!(
            inner.status,
            SessionStatus::Importing | SessionStatus::Succeeded | SessionStatus::SaveUnconfirmed
        ) {
            return Ok(None);
        }
        if inner.status != SessionStatus::Ready {
            return Err(super::login_error(LoginError::Unavailable));
        }
        let receipt = inner
            .receipt
            .as_ref()
            .ok_or_else(|| super::login_error(LoginError::Unavailable))?;
        let prepared = persistence::prepare(state, binding, receipt)?;
        let receipt = inner
            .receipt
            .take()
            .ok_or_else(|| super::login_error(LoginError::Unavailable))?;
        inner.save_target = Some(prepared.credential_id.clone());
        inner.status = SessionStatus::Importing;
        Ok(Some(SaveOperation { prepared, receipt }))
    }
}
impl SaveOperation {
    pub(super) fn run(
        self,
        state: Arc<AppState>,
        session: Arc<Session>,
    ) -> impl std::future::Future<Output = ()> + Send + 'static {
        let mut guard = SaveGuard {
            session: session.clone(),
            active: true,
        };
        async move {
            let result = tokio::time::timeout(Duration::from_secs(20), async {
                let _publication = state.lifecycle.health_control.publication.lock().await;
                // A receipt can expire while waiting for the publication owner. Never renew its lifetime.
                if self.receipt.is_expired() {
                    return Err("grok_oauth_expired");
                }
                let runtime = state
                    .route_config_runtime
                    .as_ref()
                    .ok_or("console_runtime_unavailable")?;
                runtime
                    .commit_document(
                        session.binding.revision(),
                        self.prepared.document,
                        Some("Save Grok OAuth credential".into()),
                    )
                    .await
                    .map(|snapshot| snapshot.revision().id().to_owned())
                    .map_err(|error| error.code())
            })
            .await;
            let mut inner = session.inner.lock();
            let (status, credential_id, revision, error_code) = match result {
                Ok(Ok(revision)) => (
                    SessionStatus::Succeeded,
                    Some(self.prepared.credential_id),
                    Some(revision),
                    None,
                ),
                // No automatic resubmission after admission: runtime journal recovery owns ambiguity.
                Ok(Err(code)) => (SessionStatus::SaveUnconfirmed, None, None, Some(code)),
                Err(_) => (
                    SessionStatus::SaveUnconfirmed,
                    None,
                    None,
                    Some("grok_login_save_timeout"),
                ),
            };
            inner.status = status;
            inner.saved = Some(SaveReceipt {
                credential_id,
                revision,
                error_code,
                finished: Instant::now(),
            });
            // Capture the whole Drop guard, not merely its `active` field in the async closure.
            guard.disarm();
        }
    }
}
struct SaveGuard {
    session: Arc<Session>,
    active: bool,
}
impl SaveGuard {
    fn disarm(&mut self) {
        self.active = false;
    }
}
impl Drop for SaveGuard {
    fn drop(&mut self) {
        if self.active {
            let mut inner = self.session.inner.lock();
            inner.status = SessionStatus::SaveUnconfirmed;
            inner.saved = Some(SaveReceipt {
                credential_id: None,
                revision: None,
                error_code: Some("grok_login_save_unconfirmed"),
                finished: Instant::now(),
            });
        }
    }
}
