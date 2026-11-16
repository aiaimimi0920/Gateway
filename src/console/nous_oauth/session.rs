//! Bounded initial-grant custody. Cancellation forbids publication, not receipt collection.
use super::{Binding, LoginError};
use crate::protocol::nous_portal::{DeviceChallenge, OAuthClient, ReceivedTokens};
use parking_lot::Mutex;
use serde::Serialize;
use std::{sync::Arc, time::Duration};
use tokio::time::Instant;

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
    pub challenge: Option<DeviceChallenge>,
    pub received: Option<ReceivedTokens>,
    pub inflight: bool,
    pub saved: Option<super::save::SaveReceipt>,
    pub save_target: Option<String>,
    pub error: Option<&'static str>,
}
pub struct Session {
    id: String,
    pub(super) binding: Binding,
    pub(super) client: OAuthClient,
    pub(super) deadline: Instant,
    pub(super) stop: tokio::sync::watch::Sender<bool>,
    pub(super) inner: Mutex<Inner>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceView {
    user_code: String,
    poll_after_secs: u64,
    expires_in_secs: u64,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionView {
    pub id: String,
    pub provider_id: String,
    pub expected_revision: String,
    pub status: SessionStatus,
    pub authorization_url: Option<String>,
    pub device_authorization: Option<DeviceView>,
    pub receipt_received: bool,
    pub credential_id: Option<String>,
    pub target_credential_id: Option<String>,
    pub saved_revision: Option<String>,
    pub error_code: Option<&'static str>,
}
impl Session {
    pub(super) fn new(
        binding: Binding,
        client: OAuthClient,
        challenge: DeviceChallenge,
    ) -> Arc<Self> {
        Arc::new(Self {
            id: uuid::Uuid::new_v4().to_string(),
            binding,
            client,
            deadline: Instant::now() + challenge.remaining(),
            stop: tokio::sync::watch::channel(false).0,
            inner: Mutex::new(Inner {
                status: SessionStatus::WaitingUser,
                challenge: Some(challenge),
                received: None,
                inflight: false,
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
        ) && (Instant::now() >= self.deadline
            || inner.status == SessionStatus::Ready
                && inner
                    .received
                    .as_ref()
                    .is_some_and(|r| r.admit(&self.client, None).is_err()))
        {
            inner.status = SessionStatus::Expired;
            inner.challenge = None;
            inner.error = Some("nous_login_expired");
            self.stop.send_replace(true);
        }
    }
    pub(super) fn retain(&self) -> bool {
        let mut inner = self.inner.lock();
        self.expire(&mut inner);
        inner.inflight
            || inner.status == SessionStatus::Importing
            || Instant::now() < self.deadline + Duration::from_secs(300)
            || inner.saved.as_ref().is_some_and(|r| r.retain())
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
            authorization_url: challenge.map(|c| {
                c.verification_uri_complete
                    .clone()
                    .unwrap_or_else(|| c.verification_uri.clone())
            }),
            device_authorization: challenge.map(|c| {
                let after = c.poll_after();
                DeviceView {
                    user_code: c.user_code.clone(),
                    poll_after_secs: after
                        .as_secs()
                        .saturating_add(u64::from(after.subsec_nanos() > 0)),
                    expires_in_secs: c.remaining().as_secs(),
                }
            }),
            receipt_received: inner.received.is_some(),
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
            inner.challenge = None;
            self.stop.send_replace(true);
        }
    }
    pub(super) fn take_challenge(&self) -> Result<DeviceChallenge, LoginError> {
        let mut inner = self.inner.lock();
        self.expire(&mut inner);
        if inner.status != SessionStatus::WaitingUser {
            return Err(LoginError::Unavailable);
        }
        let challenge = inner.challenge.take().ok_or(LoginError::Unavailable)?;
        inner.status = SessionStatus::Exchanging;
        inner.inflight = true;
        Ok(challenge)
    }
}
impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("NousLoginSession { material: <redacted> }")
    }
}
