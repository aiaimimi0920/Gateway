//! Only the registry owns async work. Views expose the user challenge, never device/client secrets.
use super::Binding;
use crate::protocol::kiro_oauth::{
    AuthorizationContext, DeviceChallenge, OAuthError, TokenMaterial,
};
use parking_lot::Mutex;
use serde::Serialize;
use std::{sync::Arc, time::Duration};
use tokio::{sync::watch, time::Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    Registering,
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
    pub uri: String,
    pub complete: Option<String>,
    pub code: String,
    pub interval: u64,
    pub deadline: Instant,
}
pub(super) struct Inner {
    pub status: SessionStatus,
    pub device: Option<DeviceChallenge>,
    pub challenge: Option<ChallengeView>,
    pub context: Option<AuthorizationContext>,
    pub material: Option<TokenMaterial>,
    pub saved: Option<super::save::SaveReceipt>,
    pub save_target: Option<String>,
    pub error: Option<OAuthError>,
}
pub struct Session {
    id: String,
    pub(super) binding: Binding,
    pub(super) machine: String,
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
    pub verification_uri: Option<String>,
    pub verification_uri_complete: Option<String>,
    pub user_code: Option<String>,
    pub poll_interval_secs: Option<u64>,
    pub expires_in_secs: Option<u64>,
    pub credential_id: Option<String>,
    pub target_credential_id: Option<String>,
    pub saved_revision: Option<String>,
    pub error_code: Option<String>,
}
impl Session {
    pub(super) fn new(binding: Binding, machine: String, ttl: Duration) -> Arc<Self> {
        Arc::new(Self {
            id: uuid::Uuid::new_v4().to_string(),
            binding,
            machine,
            deadline: Instant::now() + ttl,
            cancel: watch::channel(false).0,
            inner: Mutex::new(Inner {
                status: SessionStatus::Registering,
                device: None,
                challenge: None,
                context: None,
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
    pub(crate) fn machine(&self) -> &str {
        &self.machine
    }
    pub(super) fn expire(&self, inner: &mut Inner) {
        let active = matches!(
            inner.status,
            SessionStatus::Registering
                | SessionStatus::WaitingUser
                | SessionStatus::Polling
                | SessionStatus::Ready
        );
        let device_expired = matches!(
            inner.status,
            SessionStatus::WaitingUser | SessionStatus::Polling
        ) && inner
            .challenge
            .as_ref()
            .is_some_and(|c| Instant::now() >= c.deadline);
        if active && (Instant::now() >= self.deadline || device_expired) {
            inner.status = SessionStatus::Expired;
            clear_material(inner);
            self.cancel.send_replace(true);
        }
    }
    pub(super) fn retain(&self) -> bool {
        let mut inner = self.inner.lock();
        self.expire(&mut inner);
        // Bounded terminal receipts remain readable until the original session deadline.
        inner.status == SessionStatus::Importing
            || Instant::now() < self.deadline
            || inner.saved.as_ref().is_some_and(|s| s.retain())
    }
    pub fn view(&self) -> SessionView {
        let mut inner = self.inner.lock();
        self.expire(&mut inner);
        let challenge = inner.challenge.as_ref();
        SessionView {
            id: self.id.clone(),
            provider_id: self.binding.provider.clone(),
            expected_revision: self.binding.revision.clone(),
            status: inner.status,
            verification_uri: challenge.map(|c| c.uri.clone()),
            verification_uri_complete: challenge.and_then(|c| c.complete.clone()),
            user_code: challenge.map(|c| c.code.clone()),
            poll_interval_secs: challenge.map(|c| c.interval),
            expires_in_secs: challenge.map(|c| {
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
            SessionStatus::Registering
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
    inner.device = None;
    inner.challenge = None;
    inner.context = None;
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
        f.write_str("KiroLoginSession { material: <redacted> }")
    }
}
