//! One consumed PKCE attempt; exchange cancellation cannot resurrect material or start another send.
use super::{Binding, LoginError};
use crate::protocol::accio_oauth::{
    AuthorizationAttempt, LoginSite, OAuthClient, OAuthError, TokenMaterial,
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
pub(super) struct Inner {
    pub(super) status: SessionStatus,
    auth: Option<AuthorizationAttempt>,
    pub(super) material: Option<TokenMaterial>,
    pub(super) saved: Option<super::save::SaveReceipt>,
    pub(super) save_target: Option<String>,
}
pub struct Session {
    id: String,
    pub(super) binding: Binding,
    pub(super) client_id: String,
    pub(super) utdid: String,
    deadline: Instant,
    cancel: watch::Sender<bool>,
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
    pub credential_id: Option<String>,
    pub target_credential_id: Option<String>,
    pub saved_revision: Option<String>,
    pub error_code: Option<&'static str>,
}
impl Session {
    pub(super) fn new(
        binding: Binding,
        client: OAuthClient,
        site: LoginSite,
        port: u16,
        utdid: String,
        ttl: Duration,
    ) -> Result<Arc<Self>, LoginError> {
        super::target::validate_utdid(&utdid)?;
        let client_id = client.client_id().to_owned();
        let auth = AuthorizationAttempt::new(client, site, port)?;
        Ok(Arc::new(Self {
            id: uuid::Uuid::new_v4().to_string(),
            binding,
            client_id,
            utdid,
            deadline: Instant::now() + ttl,
            cancel: watch::channel(false).0,
            inner: Mutex::new(Inner {
                status: SessionStatus::WaitingUser,
                auth: Some(auth),
                material: None,
                saved: None,
                save_target: None,
            }),
        }))
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
            inner.auth = None;
            inner.material = None;
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
        ) || inner.saved.as_ref().is_some_and(|s| s.retain())
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
            inner.material = None;
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
            let auth = inner.auth.take().ok_or(LoginError::Unavailable)?;
            inner.status = SessionStatus::Exchanging;
            auth
        };
        // The protocol consumes even an invalid callback. Do not leave a reusable attempt behind.
        let mut guard = ExchangeGuard {
            session: self,
            active: true,
        };
        let grant = auth.finish(callback)?;
        let mut stop = self.cancel.subscribe();
        let result = tokio::select! {
            biased;
            _=wait_stopped(&mut stop)=>Err(LoginError::Unavailable),
            _=tokio::time::sleep_until(self.deadline)=>Err(LoginError::OAuth(OAuthError::Expired)),
            result=grant.exchange(http)=>result.map_err(LoginError::from),
        };
        let mut inner = self.inner.lock();
        self.expire(&mut inner);
        if inner.status != SessionStatus::Exchanging {
            return Err(LoginError::Unavailable);
        }
        guard.active = false;
        match result {
            // Absolute token expiry does not get renewed here. Preserve returned refresh material,
            // including an already-expired access token; ordinary dispatch still enforces expiry.
            Ok(material) => {
                inner.material = Some(material);
                inner.status = SessionStatus::Ready;
                Ok(())
            }
            Err(error) => {
                inner.status = SessionStatus::Failed;
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
            }
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
        f.write_str("AccioLoginSession { material: <redacted> }")
    }
}
