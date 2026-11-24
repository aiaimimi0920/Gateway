//! Owner-bound Amazon Quick OAuth session state and non-secret view projection.

use super::Binding;
use crate::protocol::quick_oauth::{AuthorizedSession, LoginStepOutcome, OAuthError, PendingLogin};
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LoginStep {
    Discovery,
    Authorize,
}

pub(super) struct Inner {
    pub status: SessionStatus,
    pub step: LoginStep,
    pub authorization_url: String,
    pub material: Option<AuthorizedSession>,
    pub account_summary: Option<String>,
    pub saved: Option<super::save::SaveReceipt>,
    pub save_target: Option<String>,
    pub error: Option<&'static str>,
}

pub struct Session {
    id: String,
    pub(super) binding: Binding,
    pending: PendingLogin,
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
    pub step: LoginStep,
    pub authorization_url: Option<String>,
    pub redirect_uri: String,
    pub account_summary: Option<String>,
    pub credential_id: Option<String>,
    pub target_credential_id: Option<String>,
    pub saved_revision: Option<String>,
    pub error_code: Option<&'static str>,
}

impl Session {
    pub(super) fn new(binding: Binding, pending: PendingLogin) -> Arc<Self> {
        let auth_url = pending.discovery_url().to_owned();
        let redirect = pending.redirect_uri().to_owned();
        Arc::new(Self {
            id: uuid::Uuid::new_v4().to_string(),
            binding,
            pending,
            redirect,
            deadline: Instant::now() + Duration::from_secs(600),
            stop: watch::channel(false).0,
            inner: Mutex::new(Inner {
                status: SessionStatus::WaitingUser,
                step: LoginStep::Discovery,
                authorization_url: auth_url,
                material: None,
                account_summary: None,
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

    pub(super) fn pending(&self) -> &PendingLogin {
        &self.pending
    }

    pub(super) fn set_authorize_url(&self, authorize_url: String) {
        let mut inner = self.inner.lock();
        if inner.status == SessionStatus::WaitingUser {
            inner.step = LoginStep::Authorize;
            inner.authorization_url = authorize_url;
        }
    }

    pub(super) fn complete_authorized(&self, authorized: AuthorizedSession) {
        let mut inner = self.inner.lock();
        if !matches!(
            inner.status,
            SessionStatus::WaitingUser | SessionStatus::Exchanging
        ) {
            return;
        }
        inner.account_summary = summarize_account(&authorized);
        inner.material = Some(authorized);
        inner.status = SessionStatus::Ready;
        inner.error = None;
        self.stop.send_replace(true);
    }

    pub(super) fn fail_oauth(&self, err: OAuthError) {
        let mut inner = self.inner.lock();
        if matches!(
            inner.status,
            SessionStatus::WaitingUser | SessionStatus::Exchanging
        ) {
            inner.status = if err == OAuthError::Expired {
                SessionStatus::Expired
            } else {
                SessionStatus::Failed
            };
            inner.material = None;
            inner.error = Some(classify_code(err));
            self.stop.send_replace(true);
        }
    }

    pub(crate) async fn submit_manual_callback(
        &self,
        callback_url: &str,
    ) -> Result<SessionView, crate::error::GatewayError> {
        {
            let mut inner = self.inner.lock();
            self.expire(&mut inner);
            if inner.status != SessionStatus::WaitingUser {
                return Err(super::login_error(super::LoginError::Unavailable));
            }
        }
        match self.pending.submit_callback_url(callback_url).await {
            Ok(LoginStepOutcome::AuthorizeRedirect { authorize_url }) => {
                self.set_authorize_url(authorize_url);
                Ok(self.view())
            }
            Ok(LoginStepOutcome::Completed(authorized)) => {
                self.complete_authorized(authorized);
                Ok(self.view())
            }
            Err(err) => {
                if matches!(err, OAuthError::Expired | OAuthError::AccessDenied) {
                    self.fail_oauth(err);
                }
                Err(super::login_error(err.into()))
            }
        }
    }

    pub(super) fn expire(&self, inner: &mut Inner) {
        if matches!(
            inner.status,
            SessionStatus::WaitingUser | SessionStatus::Exchanging | SessionStatus::Ready
        ) && Instant::now() >= self.deadline
        {
            inner.status = SessionStatus::Expired;
            inner.material = None;
            inner.error = Some("quick_oauth_login_expired");
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
            step: inner.step,
            authorization_url: (inner.status == SessionStatus::WaitingUser)
                .then(|| inner.authorization_url.clone()),
            redirect_uri: self.redirect.clone(),
            account_summary: inner.account_summary.clone(),
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

fn summarize_account(authorized: &AuthorizedSession) -> Option<String> {
    authorized
        .context
        .email()
        .or_else(|| authorized.context.display_name())
        .or_else(|| authorized.context.account_name())
        .or_else(|| authorized.context.aws_account_id())
        .map(str::to_owned)
}

fn classify_code(err: OAuthError) -> &'static str {
    match err {
        OAuthError::AccessDenied => "quick_oauth_access_denied",
        OAuthError::StateMismatch => "quick_oauth_state_mismatch",
        OAuthError::Expired => "quick_oauth_login_expired",
        _ => "quick_oauth_login_failed",
    }
}

impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("QuickLoginSession { material: <redacted> }")
    }
}
