//! One consumed code attempt, with client secrets erased from temporary session state on exit.
use super::{Binding, LoginError};
use crate::protocol::vertex_oauth::{
    AuthorizationAttempt, AuthorizationContext, OAuthClient, OAuthError, TokenMaterial,
    VertexTarget,
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
    pub material: TokenMaterial,
    pub context: AuthorizationContext,
}
pub(super) struct Inner {
    pub status: SessionStatus,
    auth: Option<AuthorizationAttempt>,
    client: Option<OAuthClient>,
    pub material: Option<Ready>,
    pub saved: Option<super::save::SaveReceipt>,
    pub save_target: Option<String>,
    error: Option<&'static str>,
}
pub struct Session {
    id: String,
    pub(super) binding: Binding,
    pub(super) target: VertexTarget,
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
    pub status: SessionStatus,
    pub authorization_url: Option<String>,
    pub redirect_uri: String,
    pub credential_id: Option<String>,
    pub target_credential_id: Option<String>,
    pub saved_revision: Option<String>,
    pub error_code: Option<&'static str>,
}
impl Session {
    pub(super) fn new(
        binding: Binding,
        client: OAuthClient,
        target: VertexTarget,
        port: u16,
        ttl: Duration,
    ) -> Result<Arc<Self>, LoginError> {
        target.validate()?;
        let auth = AuthorizationAttempt::new(client.clone(), port)?;
        Ok(Arc::new(Self {
            id: uuid::Uuid::new_v4().to_string(),
            binding,
            target,
            redirect: auth.redirect_uri().into(),
            deadline: Instant::now() + ttl,
            stop: watch::channel(false).0,
            inner: Mutex::new(Inner {
                status: SessionStatus::WaitingUser,
                auth: Some(auth),
                client: Some(client),
                material: None,
                saved: None,
                save_target: None,
                error: None,
            }),
        }))
    }
    pub fn id(&self) -> &str {
        &self.id
    }
    pub(crate) fn binding(&self) -> &Binding {
        &self.binding
    }
    pub(crate) fn target(&self) -> &VertexTarget {
        &self.target
    }
    pub(super) fn expire(&self, inner: &mut Inner) {
        if matches!(
            inner.status,
            SessionStatus::WaitingUser | SessionStatus::Exchanging | SessionStatus::Ready
        ) && Instant::now() >= self.deadline
        {
            inner.status = SessionStatus::Expired;
            inner.auth = None;
            inner.client = None;
            inner.material = None;
            inner.error = Some("vertex_login_expired");
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
            status: inner.status,
            authorization_url: inner.auth.as_ref().map(|a| a.authorize_url()),
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
            inner.auth = None;
            inner.client = None;
            inner.material = None;
        }
        self.stop.send_replace(true);
    }
    pub(super) fn fail_waiting(&self) {
        let mut inner = self.inner.lock();
        if inner.status == SessionStatus::WaitingUser {
            inner.status = SessionStatus::Failed;
            inner.auth = None;
            inner.client = None;
            inner.error = Some("vertex_login_target_changed");
            self.stop.send_replace(true);
        }
    }
    pub async fn complete(&self, http: &rquest::Client, callback: &str) -> Result<(), LoginError> {
        let (auth, client) = {
            let mut inner = self.inner.lock();
            self.expire(&mut inner);
            if inner.status != SessionStatus::WaitingUser {
                return Err(LoginError::Unavailable);
            }
            let auth = inner.auth.take().ok_or(LoginError::Unavailable)?;
            let client = inner.client.take().ok_or(LoginError::Unavailable)?;
            inner.status = SessionStatus::Exchanging;
            (auth, client)
        };
        // Invalid callbacks also consume the attempt. Drop of a caller/exchange never restores it.
        let mut guard = ExchangeGuard {
            session: self,
            active: true,
        };
        let grant = auth.finish(callback)?;
        let mut stop = self.stop.subscribe();
        let result = tokio::select! { biased;
            _=wait_stopped(&mut stop)=>Err(LoginError::Unavailable),
            _=tokio::time::sleep_until(self.deadline)=>Err(LoginError::OAuth(OAuthError::Expired)),
            result=grant.exchange(http)=>result.map_err(LoginError::from),
        };
        let result = result.and_then(|material| {
            Ok(Ready {
                context: AuthorizationContext::from_authorization(
                    client,
                    self.target.clone(),
                    &material,
                )?,
                material,
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
                inner.error = Some("vertex_login_exchange_failed");
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
                inner.client = None;
                inner.material = None;
                inner.error = Some("vertex_login_exchange_unconfirmed");
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
        f.write_str("VertexLoginSession { material: <redacted> }")
    }
}
