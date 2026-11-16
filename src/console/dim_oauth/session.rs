//! The validated callback consumes one code; wrong-state traffic cannot consume a pending attempt.
use super::{Binding, LoginError};
use crate::protocol::dim_oauth::{
    Authorization, AuthorizationContext, GrantReceipt, IdentityAssurance, OAuthError, Site,
};
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
    pub receipt: GrantReceipt,
    pub context: AuthorizationContext,
}
pub(super) struct Inner {
    pub status: SessionStatus,
    auth: Option<Authorization>,
    pub material: Option<Ready>,
    pub saved: Option<super::save::SaveReceipt>,
    pub save_target: Option<String>,
    error: Option<&'static str>,
}
pub struct Session {
    id: String,
    pub(super) binding: Binding,
    pub(super) redirect: String,
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
    pub site: Site,
    pub status: SessionStatus,
    pub authorization_url: Option<String>,
    pub redirect_uri: String,
    pub identity_assurance: Option<IdentityAssurance>,
    pub credential_id: Option<String>,
    pub target_credential_id: Option<String>,
    pub saved_revision: Option<String>,
    pub error_code: Option<&'static str>,
}
impl Session {
    pub(super) fn new(binding: Binding, auth: Authorization, ttl: Duration) -> Arc<Self> {
        let deadline = Instant::now() + ttl.min(auth.remaining());
        Arc::new(Self {
            id: uuid::Uuid::new_v4().to_string(),
            redirect: auth.redirect_uri().into(),
            binding,
            deadline,
            stop: watch::channel(false).0,
            inner: Mutex::new(Inner {
                status: SessionStatus::WaitingUser,
                auth: Some(auth),
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
        ) && (Instant::now() >= self.deadline
            || inner
                .material
                .as_ref()
                .is_some_and(|r| r.receipt.is_expired()))
        {
            inner.status = SessionStatus::Expired;
            inner.auth = None;
            inner.material = None;
            inner.error = Some("dim_login_expired");
            self.stop.send_replace(true);
        }
    }
    pub(super) fn retain(&self) -> bool {
        let mut inner = self.inner.lock();
        self.expire(&mut inner);
        matches!(
            inner.status,
            SessionStatus::WaitingUser
                | SessionStatus::Exchanging
                | SessionStatus::Ready
                | SessionStatus::Importing
        ) || inner.saved.as_ref().is_some_and(|r| r.retain())
    }
    pub fn view(&self) -> SessionView {
        let mut inner = self.inner.lock();
        self.expire(&mut inner);
        SessionView {
            id: self.id.clone(),
            provider_id: self.binding.provider.clone(),
            expected_revision: self.binding.revision.clone(),
            site: self.binding.site,
            status: inner.status,
            authorization_url: inner.auth.as_ref().map(|a| a.authorize_url()),
            redirect_uri: self.redirect.clone(),
            identity_assurance: inner
                .material
                .as_ref()
                .map(|r| r.receipt.identity_assurance()),
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
            inner.auth = None;
            inner.material = None;
        }
        self.stop.send_replace(true);
    }
    pub(super) fn fail_waiting(&self) {
        let mut inner = self.inner.lock();
        if inner.status == SessionStatus::WaitingUser {
            inner.status = SessionStatus::Failed;
            inner.auth = None;
            inner.error = Some("dim_login_target_changed");
            self.stop.send_replace(true);
        }
    }
    pub async fn complete(&self, http: &rquest::Client, callback: &str) -> Result<(), LoginError> {
        let grant = {
            let mut inner = self.inner.lock();
            self.expire(&mut inner);
            if inner.status != SessionStatus::WaitingUser {
                return Err(LoginError::Unavailable);
            }
            let auth = inner.auth.as_ref().ok_or(LoginError::Unavailable)?;
            if let Err(error) = auth.callback_code(callback) {
                // A denial with matching state is terminal; malformed or unrelated traffic is not.
                if matches!(error, OAuthError::AccessDenied | OAuthError::Rejected) {
                    inner.status = SessionStatus::Failed;
                    inner.auth = None;
                    inner.error = Some("dim_login_authorization_denied");
                    self.stop.send_replace(true);
                }
                return Err(error.into());
            }
            let auth = inner.auth.take().ok_or(LoginError::Unavailable)?;
            let grant = match auth.into_code_grant(callback) {
                Ok(grant) => grant,
                Err(error) => {
                    // The deadline can pass between validation and transfer; never leave an empty pending attempt.
                    inner.status = SessionStatus::Expired;
                    inner.error = Some("dim_login_expired");
                    self.stop.send_replace(true);
                    return Err(error.into());
                }
            };
            inner.status = SessionStatus::Exchanging;
            grant
        };
        let mut guard = ExchangeGuard {
            session: self,
            active: true,
        };
        let mut stop = self.stop.subscribe();
        let result = tokio::select! { biased;
            _=wait_stopped(&mut stop)=>Err(LoginError::Unavailable),
            _=tokio::time::sleep_until(self.deadline)=>Err(LoginError::OAuth(OAuthError::Expired)),
            result=grant.exchange(http)=>result.map_err(LoginError::from),
        }
        .and_then(|receipt| {
            Ok(Ready {
                context: AuthorizationContext::from_receipt(&receipt)?,
                receipt,
            })
        });
        let mut inner = self.inner.lock();
        self.expire(&mut inner);
        if inner.status != SessionStatus::Exchanging {
            return Err(LoginError::Unavailable);
        }
        guard.active = false;
        self.stop.send_replace(true);
        match result {
            Ok(ready) => {
                inner.material = Some(ready);
                inner.status = SessionStatus::Ready;
                Ok(())
            }
            Err(error) => {
                inner.status = SessionStatus::Failed;
                inner.error = Some("dim_login_exchange_failed");
                Err(error)
            }
        }
    }
}
struct ExchangeGuard<'a> {
    session: &'a Session,
    active: bool,
}
impl Drop for ExchangeGuard<'_> {
    fn drop(&mut self) {
        if self.active {
            let mut inner = self.session.inner.lock();
            if inner.status == SessionStatus::Exchanging {
                inner.status = SessionStatus::Failed;
                inner.auth = None;
                inner.material = None;
                inner.error = Some("dim_login_exchange_unconfirmed");
                self.session.stop.send_replace(true);
            }
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
        f.write_str("DimLoginSession { material: <redacted> }")
    }
}
