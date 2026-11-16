//! One owner-bound browser attempt and a bounded, pollable save receipt; views contain no tokens.
use super::Binding;
use crate::protocol::lobsterai::{PendingLogin, SessionMaterial};
use parking_lot::Mutex;
use serde::Serialize;
use std::{sync::Arc, time::Duration};
use tokio::{sync::watch, time::Instant};
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    WaitingUser,
    Exchanging,
    Ready,
    Importing,
    Succeeded,
    SaveUnconfirmed,
    Failed,
    Cancelled,
    Expired,
}
pub(super) struct Inner {
    pub status: SessionStatus,
    pub material: Option<SessionMaterial>,
    pub saved: Option<super::save::SaveReceipt>,
    pub save_target: Option<String>,
    pub error: Option<&'static str>,
}
pub struct Session {
    id: String,
    pub(super) binding: Binding,
    authorization_url: String,
    redirect: String,
    pub(super) deadline: Instant,
    pub(super) stop: watch::Sender<bool>,
    pub(super) inner: Mutex<Inner>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionView {
    pub id: String,
    pub provider_id: String,
    pub expected_revision: String,
    pub status: SessionStatus,
    pub authorization_url: Option<String>,
    pub redirect_uri: String,
    pub credential_id: Option<String>,
    pub target_credential_id: Option<String>,
    pub saved_revision: Option<String>,
    pub error_code: Option<&'static str>,
}
impl Session {
    pub(super) fn new(binding: Binding, pending: &PendingLogin) -> Arc<Self> {
        Arc::new(Self {
            id: uuid::Uuid::new_v4().to_string(),
            binding,
            authorization_url: pending.authorization_url(),
            redirect: pending.redirect_uri().into(),
            deadline: Instant::now() + Duration::from_secs(300),
            stop: watch::channel(false).0,
            inner: Mutex::new(Inner {
                status: SessionStatus::WaitingUser,
                material: None,
                saved: None,
                save_target: None,
                error: None,
            }),
        })
    }
    pub fn id(&self) -> &str {
        &self.id
    }
    pub(crate) fn binding(&self) -> &Binding {
        &self.binding
    }
    pub(super) fn expire(&self, inner: &mut Inner) {
        if matches!(
            inner.status,
            SessionStatus::WaitingUser | SessionStatus::Exchanging | SessionStatus::Ready
        ) && Instant::now() >= self.deadline
        {
            inner.status = SessionStatus::Expired;
            inner.material = None;
            inner.error = Some("lobsterai_login_expired");
            self.stop.send_replace(true);
        }
    }
    pub(super) fn retain(&self) -> bool {
        let mut inner = self.inner.lock();
        self.expire(&mut inner);
        inner.status == SessionStatus::Importing
            || inner.saved.as_ref().is_some_and(|r| r.retain())
            || Instant::now() < self.deadline + Duration::from_secs(300)
    }
    pub fn view(&self) -> SessionView {
        let mut inner = self.inner.lock();
        self.expire(&mut inner);
        SessionView {
            id: self.id.clone(),
            provider_id: self.binding.provider.clone(),
            expected_revision: self.binding.revision.clone(),
            status: inner.status,
            authorization_url: (inner.status == SessionStatus::WaitingUser)
                .then(|| self.authorization_url.clone()),
            redirect_uri: self.redirect.clone(),
            credential_id: inner.saved.as_ref().and_then(|r| r.credential_id.clone()),
            target_credential_id: inner.save_target.clone(),
            saved_revision: inner.saved.as_ref().and_then(|r| r.revision.clone()),
            error_code: inner
                .saved
                .as_ref()
                .and_then(|r| r.error_code)
                .or(inner.error),
        }
    }
    pub fn cancel(&self) {
        let mut inner = self.inner.lock();
        if matches!(
            inner.status,
            SessionStatus::WaitingUser | SessionStatus::Exchanging | SessionStatus::Ready
        ) {
            inner.status = SessionStatus::Cancelled;
            inner.material = None;
        }
        self.stop.send_replace(true);
    }
}
pub(super) async fn wait_stopped(stop: &mut watch::Receiver<bool>) {
    loop {
        if *stop.borrow_and_update() || stop.changed().await.is_err() {
            return;
        }
    }
}
impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("LobsterLoginSession { material: <redacted> }")
    }
}
