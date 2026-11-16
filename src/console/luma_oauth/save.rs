//! An accepted import survives its request; ambiguous commit is pollable and never replayed automatically.
use super::{
    persistence::{self, PreparedSave},
    Binding, LoginError, Session, SessionStatus,
};
use crate::{error::GatewayError, state::AppState};
use std::{sync::Arc, time::Duration};
use tokio::time::Instant;

pub(super) struct SaveReceipt {
    pub credential_id: Option<String>,
    pub revision: Option<String>,
    pub error_code: Option<&'static str>,
    finished: Instant,
}
impl SaveReceipt {
    pub(super) fn refreshed(
        id: Option<String>,
        revision: Option<String>,
        error: Option<&'static str>,
    ) -> Self {
        Self {
            credential_id: id,
            revision,
            error_code: error,
            finished: Instant::now(),
        }
    }
    pub(super) fn retain(&self) -> bool {
        self.finished.elapsed() < Duration::from_secs(300)
    }
}
pub(super) struct SaveOperation {
    prepared: PreparedSave,
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
        let ready = inner
            .material
            .as_ref()
            .ok_or_else(|| super::login_error(LoginError::Unavailable))?;
        let prepared = persistence::prepare(state, binding, ready, self.deadline)?;
        inner.material = None;
        inner.save_target = Some(prepared.credential_id.clone());
        inner.status = SessionStatus::Importing;
        Ok(Some(SaveOperation { prepared }))
    }
}
impl SaveOperation {
    pub(super) fn run(
        self,
        state: Arc<AppState>,
        session: Arc<Session>,
    ) -> impl std::future::Future<Output = ()> + Send + 'static {
        // The guard exists before the task is polled, so dropped admission cannot leave Importing forever.
        let mut guard = SaveGuard {
            session: session.clone(),
            active: true,
        };
        async move {
            let id = self.prepared.credential_id;
            let result = tokio::time::timeout(Duration::from_secs(20), async {
                let _publication = state.lifecycle.health_control.publication.lock().await;
                if Instant::now() >= self.prepared.valid_until {
                    return Err("luma_login_material_expired");
                }
                let runtime = state
                    .route_config_runtime
                    .as_ref()
                    .ok_or("console_runtime_unavailable")?;
                runtime
                    .commit_document(
                        session.binding.revision(),
                        self.prepared.document,
                        Some("Save Luma MCP OAuth credential".into()),
                    )
                    .await
                    .map(|s| s.revision().id().to_owned())
                    .map_err(|e| e.code())
            })
            .await;
            let (status, credential_id, revision, error_code) = match result {
                Ok(Ok(revision)) => (SessionStatus::Succeeded, Some(id), Some(revision), None),
                Ok(Err(code)) => (SessionStatus::SaveUnconfirmed, None, None, Some(code)),
                Err(_) => (
                    SessionStatus::SaveUnconfirmed,
                    None,
                    None,
                    Some("luma_login_save_timeout"),
                ),
            };
            let mut inner = session.inner.lock();
            inner.status = status;
            inner.saved = Some(SaveReceipt {
                credential_id,
                revision,
                error_code,
                finished: Instant::now(),
            });
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
        // Capture the whole guard in the future, not just its Copy field.
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
                error_code: Some("luma_login_save_unconfirmed"),
                finished: Instant::now(),
            });
        }
    }
}
