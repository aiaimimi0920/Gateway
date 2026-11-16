//! Pending material is memory-only; terminal receipts remain briefly pollable without secrets.
use super::Binding;
use crate::protocol::luma_oauth::{Authorization, AuthorizationContext, TokenMaterial};
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
pub(super) struct Ready {
    pub material: TokenMaterial,
    pub context: AuthorizationContext,
}
pub(super) struct Inner {
    pub status: SessionStatus,
    pub auth: Option<Authorization>,
    pub material: Option<Ready>,
    pub saved: Option<super::save::SaveReceipt>,
    pub save_target: Option<String>,
    pub error: Option<&'static str>,
    pub refresh_pending: Option<(
        crate::console::refresh_grants::GrantRecord,
        crate::console::refresh_grants::GrantReceipt,
    )>,
    finished: Option<Instant>,
}
pub struct Session {
    id: String,
    pub(super) binding: Binding,
    pub(super) deadline: Instant,
    pub(super) stop: watch::Sender<bool>,
    pub(super) inner: Mutex<Inner>,
    pub(super) refresh: Option<super::refresh::RefreshTarget>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionView {
    pub id: String,
    pub provider_id: String,
    pub offline_access: bool,
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
    pub(super) fn new(binding: Binding, auth: Authorization, ttl: Duration) -> Arc<Self> {
        Arc::new(Self {
            refresh: None,
            id: uuid::Uuid::new_v4().to_string(),
            deadline: Instant::now() + ttl.min(auth.remaining()),
            binding,
            stop: watch::channel(false).0,
            inner: Mutex::new(Inner {
                status: SessionStatus::WaitingUser,
                auth: Some(auth),
                material: None,
                saved: None,
                save_target: None,
                error: None,
                refresh_pending: None,
                finished: None,
            }),
        })
    }
    pub(super) fn new_refresh(
        binding: Binding,
        target: super::refresh::RefreshTarget,
    ) -> Arc<Self> {
        Arc::new(Self {
            id: uuid::Uuid::new_v4().to_string(),
            deadline: Instant::now() + Duration::from_secs(1200),
            stop: watch::channel(false).0,
            refresh: Some(target),
            inner: Mutex::new(Inner {
                status: SessionStatus::Ready,
                auth: None,
                material: None,
                saved: None,
                save_target: binding.credential.clone(),
                error: None,
                refresh_pending: None,
                finished: None,
            }),
            binding,
        })
    }
    pub fn id(&self) -> &str {
        &self.id
    }
    pub(crate) fn binding(&self) -> &Binding {
        &self.binding
    }
    pub(super) fn finish(
        &self,
        inner: &mut Inner,
        status: SessionStatus,
        error: Option<&'static str>,
    ) {
        inner.status = status;
        inner.auth = None;
        inner.material = None;
        inner.error = error;
        inner.finished = Some(Instant::now());
        self.stop.send_replace(true);
    }
    pub(super) fn expire(&self, inner: &mut Inner) {
        if matches!(
            inner.status,
            SessionStatus::WaitingUser | SessionStatus::Exchanging | SessionStatus::Ready
        ) && (Instant::now() >= self.deadline
            || inner.material.as_ref().is_some_and(|r| {
                r.material
                    .expires_at()
                    .is_some_and(|at| at <= time::OffsetDateTime::now_utc())
            }))
        {
            self.finish(inner, SessionStatus::Expired, Some("luma_login_expired"));
        }
    }
    pub(super) fn retain(&self) -> bool {
        let mut inner = self.inner.lock();
        self.expire(&mut inner);
        inner.refresh_pending.is_some()
            || matches!(
                inner.status,
                SessionStatus::WaitingUser
                    | SessionStatus::Exchanging
                    | SessionStatus::Ready
                    | SessionStatus::Importing
            )
            || inner.saved.as_ref().is_some_and(|r| r.retain())
            || inner
                .finished
                .is_some_and(|at| at.elapsed() < Duration::from_secs(300))
    }
    pub fn view(&self) -> SessionView {
        let mut inner = self.inner.lock();
        self.expire(&mut inner);
        SessionView {
            id: self.id.clone(),
            provider_id: self.binding.provider.clone(),
            offline_access: self.binding.offline_access,
            expected_revision: self.binding.revision.clone(),
            status: inner.status,
            authorization_url: inner.auth.as_ref().map(Authorization::authorize_url),
            redirect_uri: self.binding.redirect.clone(),
            target_credential_id: inner.save_target.clone(),
            credential_id: inner.saved.as_ref().and_then(|r| r.credential_id.clone()),
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
            self.finish(&mut inner, SessionStatus::Cancelled, None);
        }
        self.stop.send_replace(true);
    }
    pub(super) fn fail_waiting(&self) {
        let mut inner = self.inner.lock();
        if inner.status == SessionStatus::WaitingUser {
            self.finish(
                &mut inner,
                SessionStatus::Failed,
                Some("luma_login_target_changed"),
            );
        }
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
        f.write_str("LumaLoginSession { material: <redacted> }")
    }
}
