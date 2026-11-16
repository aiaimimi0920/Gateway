//! A bounded capability view; authorization tokens and device receipt never cross this wire boundary.
use super::Binding;
use crate::protocol::freebuff_login::{Credentials, LoginChallenge, LoginError};
use parking_lot::Mutex;
use serde::Serialize;
use std::{sync::Arc, time::Duration};
use tokio::{sync::watch, time::Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    Starting,
    WaitingUser,
    Polling,
    Ready,
    Importing,
    Succeeded,
    SaveUnconfirmed,
    Failed,
    Cancelled,
    Expired,
}
pub(super) struct ChallengeView {
    pub url: String,
    pub interval: u64,
    pub deadline: Instant,
}
pub(super) struct Inner {
    pub status: SessionStatus,
    pub challenge: Option<LoginChallenge>,
    pub view: Option<ChallengeView>,
    pub material: Option<Credentials>,
    pub saved: Option<super::save::SaveReceipt>,
    pub save_target: Option<String>,
    pub error: Option<LoginError>,
}
pub struct Session {
    id: String,
    pub(super) binding: Binding,
    pub(super) fingerprint: String,
    pub(super) deadline: Instant,
    pub(super) cancel: watch::Sender<bool>,
    pub(super) inner: Mutex<Inner>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionView {
    pub id: String,
    pub provider_id: String,
    pub expected_revision: String,
    pub status: SessionStatus,
    pub login_url: Option<String>,
    pub poll_interval_secs: Option<u64>,
    pub expires_in_secs: Option<u64>,
    pub credential_id: Option<String>,
    pub target_credential_id: Option<String>,
    pub saved_revision: Option<String>,
    pub error_code: Option<String>,
}
impl Session {
    pub(super) fn new(binding: Binding, fingerprint: String, ttl: Duration) -> Arc<Self> {
        Arc::new(Self {
            id: uuid::Uuid::new_v4().to_string(),
            binding,
            fingerprint,
            deadline: Instant::now() + ttl,
            cancel: watch::channel(false).0,
            inner: Mutex::new(Inner {
                status: SessionStatus::Starting,
                challenge: None,
                view: None,
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
    pub(crate) fn fingerprint(&self) -> &str {
        &self.fingerprint
    }
    pub(super) fn expire(&self, inner: &mut Inner) {
        if matches!(
            inner.status,
            SessionStatus::Starting
                | SessionStatus::WaitingUser
                | SessionStatus::Polling
                | SessionStatus::Ready
        ) && (Instant::now() >= self.deadline
            || inner
                .view
                .as_ref()
                .is_some_and(|c| Instant::now() >= c.deadline))
        {
            inner.status = SessionStatus::Expired;
            clear_material(inner);
            self.cancel.send_replace(true);
        }
    }
    pub(super) fn retain(&self) -> bool {
        let mut inner = self.inner.lock();
        self.expire(&mut inner);
        inner.status == SessionStatus::Importing
            || Instant::now() < self.deadline
            || inner.saved.as_ref().is_some_and(|s| s.retain())
    }
    pub fn view(&self) -> SessionView {
        let mut inner = self.inner.lock();
        self.expire(&mut inner);
        SessionView {
            id: self.id.clone(),
            provider_id: self.binding.provider.clone(),
            expected_revision: self.binding.revision.clone(),
            status: inner.status,
            login_url: inner.view.as_ref().map(|c| c.url.clone()),
            poll_interval_secs: inner.view.as_ref().map(|c| c.interval),
            expires_in_secs: inner.view.as_ref().map(|c| {
                c.deadline
                    .saturating_duration_since(Instant::now())
                    .as_secs()
            }),
            credential_id: inner.saved.as_ref().and_then(|s| s.credential_id.clone()),
            target_credential_id: inner.save_target.clone(),
            saved_revision: inner.saved.as_ref().and_then(|s| s.revision.clone()),
            error_code: inner
                .saved
                .as_ref()
                .and_then(|s| s.error_code.map(str::to_owned))
                .or_else(|| inner.error.map(|e| e.to_string())),
        }
    }
    pub fn cancel(&self) {
        let mut inner = self.inner.lock();
        if matches!(
            inner.status,
            SessionStatus::Starting
                | SessionStatus::WaitingUser
                | SessionStatus::Polling
                | SessionStatus::Ready
        ) {
            inner.status = SessionStatus::Cancelled;
            clear_material(&mut inner);
        }
        self.cancel.send_replace(true);
    }
}
pub(super) fn clear_material(inner: &mut Inner) {
    inner.challenge = None;
    inner.view = None;
    inner.material = None;
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
        f.write_str("FreeBuffLoginSession { material: <redacted> }")
    }
}
