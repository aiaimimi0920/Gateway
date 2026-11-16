//! A registry-owned attempt survives a dropped HTTP request; unknown outcomes never restore the challenge.
use super::{Session, SessionStatus};
use crate::{
    protocol::nous_portal::{DeviceChallenge, DevicePoll},
    state::AppState,
};
use std::sync::Arc;

pub(super) fn poll(
    state: Arc<AppState>,
    session: Arc<Session>,
    mut challenge: DeviceChallenge,
) -> impl std::future::Future<Output = ()> + Send + 'static {
    let mut guard = PollGuard {
        session: session.clone(),
        active: true,
    };
    async move {
        // Wait before the attempt so cancellation/revision drift during pacing prevents the send.
        let mut stop = session.stop.subscribe();
        tokio::select! { biased;
            _ = super::registry::wait_stopped(&mut stop) => return,
            _ = tokio::time::sleep(challenge.poll_after().min(challenge.remaining())) => {},
        }
        let result = if session.view().status != SessionStatus::Exchanging
            || super::validate_target(&state, &session.binding).is_err()
        {
            Err(super::LoginError::Unavailable)
        } else {
            session
                .client
                .poll_device(state.upstream_client.client(), &mut challenge)
                .await
                .map_err(Into::into)
        };
        let mut inner = session.inner.lock();
        inner.inflight = false;
        // Retain raw delivery BEFORE eligibility, expiry, cancellation or revision decisions.
        if let Ok(DevicePoll::Received { tokens, .. }) = &result {
            inner.received = Some(tokens.clone());
        }
        session.expire(&mut inner);
        guard.disarm();
        if inner.status != SessionStatus::Exchanging {
            return;
        }
        if super::validate_target(&state, &session.binding).is_err() {
            inner.status = SessionStatus::Failed;
            inner.error = Some("nous_login_target_changed");
            return;
        }
        match result {
            Ok(DevicePoll::Pending | DevicePoll::SlowDown) => {
                inner.challenge = Some(challenge);
                inner.status = SessionStatus::WaitingUser;
            }
            Ok(DevicePoll::Received {
                tokens,
                challenge_expired,
            }) => {
                if !challenge_expired && tokens.admit(&session.client, None).is_ok() {
                    inner.status = SessionStatus::Ready;
                } else {
                    inner.status = SessionStatus::Failed;
                    inner.error = Some("nous_login_received_not_admitted");
                }
            }
            Err(_) => {
                inner.status = SessionStatus::Failed;
                inner.error = Some("nous_login_exchange_unconfirmed");
            }
        }
    }
}
struct PollGuard {
    session: Arc<Session>,
    active: bool,
}
impl PollGuard {
    // A method captures the whole Drop guard in the future, not only its active field.
    fn disarm(&mut self) {
        self.active = false;
    }
}
impl Drop for PollGuard {
    fn drop(&mut self) {
        if self.active {
            let mut inner = self.session.inner.lock();
            inner.inflight = false;
            if inner.status == SessionStatus::Exchanging {
                inner.status = SessionStatus::Failed;
                inner.error = Some("nous_login_exchange_unconfirmed");
            }
        }
    }
}
