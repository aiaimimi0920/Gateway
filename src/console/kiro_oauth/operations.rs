//! Single-step OIDC operations, admitted before spawn and joined independently of HTTP waiters.
use super::{
    session::{clear_material, wait_stopped, ChallengeView},
    LoginError, Session, SessionStatus,
};
use crate::protocol::kiro_oauth::{
    AuthorizationContext, ClientRegistration, DeviceChallenge, DevicePoll, Directory, OAuthError,
    Scope,
};
use std::sync::Arc;

impl Session {
    pub(super) fn initialize(
        self: &Arc<Self>,
        http: rquest::Client,
        directory: Directory,
        scopes: Vec<Scope>,
    ) -> impl std::future::Future<Output = ()> + Send + 'static {
        let mut guard = LoginGuard::new(self.clone(), SessionStatus::Registering);
        let session = self.clone();
        async move {
            let mut stop = session.cancel.subscribe();
            let result = tokio::select! { biased;
                _=wait_stopped(&mut stop)=>Err(OAuthError::Expired),
                _=tokio::time::sleep_until(session.deadline)=>Err(OAuthError::Expired),
                result=async {
                    let registration = ClientRegistration::register(&http, directory, &scopes).await?;
                    let context = AuthorizationContext::from_registration(&registration, &session.machine)?;
                    let device = registration.begin_device(&http).await?;
                    Ok::<_,OAuthError>((context, device))
                }=>result,
            };
            let mut inner = session.inner.lock();
            session.expire(&mut inner);
            if inner.status != SessionStatus::Registering {
                return;
            }
            guard.disarm();
            match result {
                Ok((context, device)) => {
                    inner.challenge = Some(challenge(&device, session.deadline));
                    inner.context = Some(context);
                    inner.device = Some(device);
                    inner.status = SessionStatus::WaitingUser;
                }
                Err(error) => {
                    inner.status = SessionStatus::Failed;
                    inner.error = Some(error);
                }
            }
        }
    }
    pub(super) fn begin_poll(
        self: &Arc<Self>,
        http: rquest::Client,
    ) -> Result<impl std::future::Future<Output = ()> + Send + 'static, LoginError> {
        let mut device = {
            let mut inner = self.inner.lock();
            self.expire(&mut inner);
            if inner.status != SessionStatus::WaitingUser {
                return Err(LoginError::Unavailable);
            }
            let device = inner.device.take().ok_or(LoginError::Unavailable)?;
            inner.status = SessionStatus::Polling;
            device
        };
        let mut guard = LoginGuard::new(self.clone(), SessionStatus::Polling);
        let session = self.clone();
        Ok(async move {
            let mut stop = session.cancel.subscribe();
            let result = tokio::select! { biased;
                _=wait_stopped(&mut stop)=>Err(OAuthError::Expired),
                _=tokio::time::sleep_until(session.deadline)=>Err(OAuthError::Expired),
                result=device.poll(&http)=>result,
            };
            let mut inner = session.inner.lock();
            session.expire(&mut inner);
            if inner.status != SessionStatus::Polling {
                return;
            }
            guard.disarm();
            match result {
                Ok(DevicePoll::Pending | DevicePoll::SlowDown) => {
                    inner.challenge = Some(challenge(&device, session.deadline));
                    inner.device = Some(device);
                    inner.status = SessionStatus::WaitingUser;
                }
                Ok(DevicePoll::Authorized(material)) => {
                    inner.challenge = None;
                    inner.material = Some(material);
                    inner.status = SessionStatus::Ready;
                }
                Err(error) => {
                    clear_material(&mut inner);
                    inner.status = SessionStatus::Failed;
                    inner.error = Some(error);
                }
            }
        })
    }
}
fn challenge(device: &DeviceChallenge, deadline: tokio::time::Instant) -> ChallengeView {
    ChallengeView {
        uri: device.verification_uri().into(),
        complete: device.verification_uri_complete().map(str::to_owned),
        code: device.user_code().into(),
        interval: device.interval().as_secs(),
        deadline: deadline.min(tokio::time::Instant::now() + device.remaining()),
    }
}
struct LoginGuard {
    session: Arc<Session>,
    expected: SessionStatus,
    active: bool,
}
impl LoginGuard {
    // Capture the entire Drop owner, not just a disjoint bool field in a Rust 2021 async closure.
    fn disarm(&mut self) {
        self.active = false;
    }
    fn new(session: Arc<Session>, expected: SessionStatus) -> Self {
        Self {
            session,
            expected,
            active: true,
        }
    }
}
impl Drop for LoginGuard {
    fn drop(&mut self) {
        if self.active {
            let mut inner = self.session.inner.lock();
            if inner.status == self.expected {
                clear_material(&mut inner);
                inner.status = SessionStatus::Failed;
                inner.error = Some(OAuthError::Transport);
            }
        }
    }
}
