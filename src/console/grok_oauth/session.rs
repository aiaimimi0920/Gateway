//! A callback is consumed once. Dropping an exchange fails it closed instead of leaving it busy.
use super::{Binding, LoginError};
use crate::protocol::grok_oauth::{
    Authorization, DeviceChallenge, GrantReceipt, OAuthClient, OAuthError, PrincipalConstraint,
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
    Claimed,
    Importing,
    Succeeded,
    SaveUnconfirmed,
    Failed,
    Cancelled,
    Expired,
}
pub(super) struct Inner {
    pub(super) automatic_callback: bool,
    pub(super) status: SessionStatus,
    pub(super) auth: Option<Arc<Authorization>>,
    pub(super) device: Option<DeviceChallenge>,
    pub(super) receipt: Option<GrantReceipt>,
    pub(super) saved: Option<super::save::SaveReceipt>,
    pub(super) save_target: Option<String>,
    identity_verified: bool,
}
pub struct Session {
    id: String,
    pub(super) binding: Binding,
    pub(super) client: OAuthClient,
    pub(super) constraint: Option<PrincipalConstraint>,
    pub(super) deadline: Instant,
    pub(super) cancel: watch::Sender<bool>,
    pub(super) inner: Mutex<Inner>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionView {
    pub automatic_callback: bool,
    pub id: String,
    pub provider_id: String,
    pub expected_revision: String,
    pub status: SessionStatus,
    pub authorization_url: Option<String>,
    pub device_authorization: Option<super::DeviceView>,
    pub identity_verified: bool,
    pub credential_id: Option<String>,
    pub target_credential_id: Option<String>,
    pub saved_revision: Option<String>,
    pub error_code: Option<&'static str>,
}
impl Session {
    pub(super) fn new(
        binding: Binding,
        client: OAuthClient,
        redirect: &str,
        constraint: Option<PrincipalConstraint>,
        ttl: Duration,
    ) -> Result<Arc<Self>, LoginError> {
        let auth = Arc::new(Authorization::new(&client, redirect)?);
        Ok(Self::build(binding, client, constraint, ttl, Some(auth)))
    }
    pub(super) fn build(
        binding: Binding,
        client: OAuthClient,
        constraint: Option<PrincipalConstraint>,
        ttl: Duration,
        auth: Option<Arc<Authorization>>,
    ) -> Arc<Self> {
        let status = if auth.is_some() {
            SessionStatus::WaitingUser
        } else {
            SessionStatus::Exchanging
        };
        Arc::new(Self {
            id: uuid::Uuid::new_v4().to_string(),
            binding,
            client,
            constraint,
            deadline: Instant::now() + ttl,
            cancel: watch::channel(false).0,
            inner: Mutex::new(Inner {
                automatic_callback: false,
                status,
                auth,
                device: None,
                receipt: None,
                saved: None,
                save_target: None,
                identity_verified: false,
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
                .device
                .as_ref()
                .is_some_and(|d| d.remaining().is_zero())
            || inner.receipt.as_ref().is_some_and(GrantReceipt::is_expired))
        {
            inner.status = SessionStatus::Expired;
            inner.auth = None;
            inner.device = None;
            inner.receipt = None;
            self.cancel.send_replace(true);
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
        ) || inner.saved.as_ref().is_some_and(|saved| saved.retain())
    }
    pub fn view(&self) -> SessionView {
        let mut inner = self.inner.lock();
        self.expire(&mut inner);
        SessionView {
            automatic_callback: inner.automatic_callback,
            id: self.id.clone(),
            provider_id: self.binding.provider.clone(),
            expected_revision: self.binding.revision.clone(),
            status: inner.status,
            authorization_url: inner.auth.as_ref().map(|a| a.authorize_url()).or_else(|| {
                inner.device.as_ref().map(|d| {
                    d.verification_uri_complete
                        .as_ref()
                        .unwrap_or(&d.verification_uri)
                        .clone()
                })
            }),
            device_authorization: inner.device.as_ref().map(super::DeviceView::from),
            identity_verified: inner.identity_verified,
            credential_id: inner.saved.as_ref().and_then(|s| s.credential_id.clone()),
            target_credential_id: inner.save_target.clone(),
            saved_revision: inner.saved.as_ref().and_then(|s| s.revision.clone()),
            error_code: inner.saved.as_ref().and_then(|s| s.error_code),
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
            inner.device = None;
            inner.receipt = None;
        }
        self.cancel.send_replace(true);
    }
    pub async fn complete(&self, http: &rquest::Client, callback: &str) -> Result<(), LoginError> {
        let auth = {
            let mut inner = self.inner.lock();
            self.expire(&mut inner);
            if inner.status != SessionStatus::WaitingUser {
                return Err(LoginError::Unavailable);
            }
            let auth = inner.auth.as_ref().ok_or(LoginError::Unavailable)?;
            if let Err(error) = auth.callback_code(callback) {
                if matches!(error, OAuthError::AccessDenied | OAuthError::Rejected) {
                    inner.status = SessionStatus::Failed;
                    inner.auth = None;
                }
                return Err(error.into());
            }
            let auth = inner.auth.take().ok_or(LoginError::Unavailable)?;
            inner.status = SessionStatus::Exchanging;
            auth
        };
        let mut guard = ExchangeGuard {
            session: self,
            active: true,
        };
        let mut stop = self.cancel.subscribe();
        let result = tokio::select! {
            biased;
            _=wait_stopped(&mut stop)=>Err(LoginError::Unavailable),
            _=tokio::time::sleep_until(self.deadline)=>Err(LoginError::OAuth(OAuthError::Expired)),
            result=self.client.complete_code(http,&auth,callback,self.constraint.as_ref())=>result.map_err(LoginError::from),
        };
        let mut inner = self.inner.lock();
        self.expire(&mut inner);
        if inner.status != SessionStatus::Exchanging {
            return Err(LoginError::Unavailable);
        }
        guard.active = false;
        match result {
            Ok(receipt) if !receipt.is_expired() => {
                inner.identity_verified = receipt.identity().is_some();
                inner.receipt = Some(receipt);
                inner.status = SessionStatus::Ready;
                Ok(())
            }
            Ok(_) => {
                inner.status = SessionStatus::Expired;
                Err(OAuthError::Expired.into())
            }
            Err(error) => {
                inner.status = SessionStatus::Failed;
                Err(error)
            }
        }
    }
    /// Transfers material once to a separately owned commit operation, not a success receipt.
    /// The future persistence owner must resolve normal SecretPatches and commit this exact revision.
    pub fn claim_receipt(&self, binding: &Binding) -> Result<GrantReceipt, LoginError> {
        let mut inner = self.inner.lock();
        self.expire(&mut inner);
        if self.binding != *binding || inner.status != SessionStatus::Ready {
            return Err(LoginError::Unavailable);
        }
        let receipt = inner.receipt.take().ok_or(LoginError::Unavailable)?;
        inner.status = SessionStatus::Claimed;
        Ok(receipt)
    }
}
pub(super) struct ExchangeGuard<'a> {
    pub(super) session: &'a Session,
    pub(super) active: bool,
}
impl Drop for ExchangeGuard<'_> {
    fn drop(&mut self) {
        if !self.active {
            return;
        }
        let mut inner = self.session.inner.lock();
        if inner.status == SessionStatus::Exchanging {
            inner.status = SessionStatus::Failed;
            inner.auth = None;
            inner.device = None;
            inner.receipt = None;
        }
    }
}
pub(super) async fn wait_stopped(stop: &mut watch::Receiver<bool>) {
    loop {
        if *stop.borrow_and_update() {
            return;
        }
        if stop.changed().await.is_err() {
            return;
        }
    }
}
impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("GrokLoginSession { material: <redacted> }")
    }
}
