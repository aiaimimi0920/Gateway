//! Initialization and one paced status GET are registry-owned, never detached HTTP waiter work.
use super::{
    session::{clear_material, wait_stopped, ChallengeView},
    LoginError, Session, SessionStatus,
};
use crate::protocol::freebuff_login::{
    LoginChallenge, LoginError as WireError, LoginPoll, LoginTransport,
};
use std::sync::Arc;
use tokio::time::Instant;

impl Session {
    pub(super) fn initialize(
        self: &Arc<Self>,
        http: LoginTransport,
    ) -> impl std::future::Future<Output = ()> + Send + 'static {
        let mut guard = LoginGuard::new(self.clone(), SessionStatus::Starting);
        let session = self.clone();
        async move {
            let mut stop = session.cancel.subscribe();
            let result = tokio::select! { biased;
                _=wait_stopped(&mut stop)=>Err(WireError::Cancelled),
                _=tokio::time::sleep_until(session.deadline)=>Err(WireError::Expired),
                result=LoginChallenge::start(&http, &session.fingerprint)=>result,
            };
            let mut inner = session.inner.lock();
            session.expire(&mut inner);
            if inner.status != SessionStatus::Starting {
                return;
            }
            guard.disarm();
            match result {
                Ok(challenge) => {
                    inner.view = Some(challenge_view(&challenge, session.deadline));
                    inner.challenge = Some(challenge);
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
        http: LoginTransport,
    ) -> Result<impl std::future::Future<Output = ()> + Send + 'static, LoginError> {
        let mut challenge = {
            let mut inner = self.inner.lock();
            self.expire(&mut inner);
            if inner.status != SessionStatus::WaitingUser {
                return Err(LoginError::Unavailable);
            }
            let challenge = inner.challenge.take().ok_or(LoginError::Unavailable)?;
            inner.status = SessionStatus::Polling;
            inner.error = None;
            challenge
        };
        let mut guard = LoginGuard::new(self.clone(), SessionStatus::Polling);
        let session = self.clone();
        Ok(async move {
            let mut stop = session.cancel.subscribe();
            let result = tokio::select! { biased;
                _=wait_stopped(&mut stop)=>Err(WireError::Cancelled),
                _=tokio::time::sleep_until(session.deadline)=>Err(WireError::Expired),
                result=challenge.poll(&http)=>result,
            };
            let mut inner = session.inner.lock();
            session.expire(&mut inner);
            if inner.status != SessionStatus::Polling {
                return;
            }
            guard.disarm();
            match result {
                Ok(LoginPoll::Pending) | Err(WireError::Transport | WireError::Timeout) => {
                    // A safe GET can be requested again, with the same challenge, pacing and original TTL.
                    if let Err(error) = result {
                        inner.error = Some(error);
                    }
                    inner.view = Some(challenge_view(&challenge, session.deadline));
                    inner.challenge = Some(challenge);
                    inner.status = SessionStatus::WaitingUser;
                }
                Ok(LoginPoll::Authorized(material)) => {
                    inner.view = None;
                    inner.material = Some(material);
                    inner.status = SessionStatus::Ready;
                }
                Err(error) => {
                    clear_material(&mut inner);
                    inner.status = if error == WireError::Expired {
                        SessionStatus::Expired
                    } else {
                        SessionStatus::Failed
                    };
                    inner.error = Some(error);
                }
            }
        })
    }
}
fn challenge_view(challenge: &LoginChallenge, deadline: Instant) -> ChallengeView {
    ChallengeView {
        url: challenge.login_url().into(),
        interval: challenge.interval().as_secs(),
        deadline: deadline.min(Instant::now() + challenge.remaining()),
    }
}
struct LoginGuard {
    session: Arc<Session>,
    expected: SessionStatus,
    active: bool,
}
impl LoginGuard {
    fn new(session: Arc<Session>, expected: SessionStatus) -> Self {
        Self {
            session,
            expected,
            active: true,
        }
    }
    fn disarm(&mut self) {
        self.active = false;
    }
}
impl Drop for LoginGuard {
    fn drop(&mut self) {
        if self.active {
            let mut inner = self.session.inner.lock();
            if inner.status == self.expected {
                clear_material(&mut inner);
                inner.status = SessionStatus::Failed;
                inner.error = Some(WireError::Transport);
            }
        }
    }
}
