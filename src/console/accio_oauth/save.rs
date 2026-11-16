//! A normal revision/CAS commit owns accepted material, independently of its HTTP waiting request.
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
        let material = inner
            .material
            .as_ref()
            .ok_or_else(|| super::login_error(LoginError::Unavailable))?;
        let prepared = persistence::prepare(state, binding, self, material)?;
        // PreparedSave now owns the complete, validated document. No browser receipt or second write.
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
        let mut guard = SaveGuard {
            session: session.clone(),
            active: true,
        };
        async move {
            let result = tokio::time::timeout(Duration::from_secs(20), async {
                let _publication = state.lifecycle.health_control.publication.lock().await;
                let runtime = state
                    .route_config_runtime
                    .as_ref()
                    .ok_or("console_runtime_unavailable")?;
                runtime
                    .commit_document(
                        session.binding.revision(),
                        self.prepared.document,
                        Some("Save Accio OAuth credential".into()),
                    )
                    .await
                    .map(|s| s.revision().id().to_owned())
                    .map_err(|e| e.code())
            })
            .await;
            let (status, credential_id, revision, error_code) = match result {
                Ok(Ok(revision)) => (
                    SessionStatus::Succeeded,
                    Some(self.prepared.credential_id),
                    Some(revision),
                    None,
                ),
                Ok(Err(code)) => (SessionStatus::SaveUnconfirmed, None, None, Some(code)),
                Err(_) => (
                    SessionStatus::SaveUnconfirmed,
                    None,
                    None,
                    Some("accio_login_save_timeout"),
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
                error_code: Some("accio_login_save_unconfirmed"),
                finished: Instant::now(),
            });
        }
    }
}
