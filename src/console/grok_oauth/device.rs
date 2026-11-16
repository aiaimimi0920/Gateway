//! One explicit device poll per action. The existing registry, cancellation and save own lifetime.
use super::{
    session::{wait_stopped, ExchangeGuard},
    LoginError, Session, SessionStatus,
};
use crate::protocol::grok_oauth::{DeviceChallenge, GrantPoll, OAuthError};
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceView {
    pub user_code: String,
    pub poll_interval_secs: u64,
    pub poll_after_secs: u64,
    pub expires_in_secs: u64,
}
impl From<&DeviceChallenge> for DeviceView {
    fn from(device: &DeviceChallenge) -> Self {
        let after = device.poll_after();
        Self {
            user_code: device.user_code.clone(),
            poll_interval_secs: device.interval().as_secs(),
            poll_after_secs: after.as_secs() + u64::from(after.subsec_nanos() != 0),
            expires_in_secs: device.remaining().as_secs(),
        }
    }
}
impl Session {
    pub(super) async fn begin_device(&self, http: &rquest::Client) -> Result<(), LoginError> {
        let mut guard = ExchangeGuard {
            session: self,
            active: true,
        };
        let mut stop = self.cancel.subscribe();
        let result = tokio::select! {
            biased;
            _=wait_stopped(&mut stop)=>Err(LoginError::Stopped),
            _=tokio::time::sleep_until(self.deadline)=>Err(OAuthError::Expired.into()),
            result=self.client.begin_device(http)=>result.map_err(LoginError::from),
        };
        let mut inner = self.inner.lock();
        self.expire(&mut inner);
        if inner.status != SessionStatus::Exchanging {
            return Err(LoginError::Unavailable);
        }
        guard.active = false;
        match result {
            Ok(device) if !device.remaining().is_zero() => {
                inner.device = Some(device);
                inner.status = SessionStatus::WaitingUser;
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
    pub async fn poll_device(&self, http: &rquest::Client) -> Result<(), LoginError> {
        let mut device = {
            let mut inner = self.inner.lock();
            self.expire(&mut inner);
            if inner.status != SessionStatus::WaitingUser {
                return Err(LoginError::Unavailable);
            }
            // A legitimate long interval/slow_down must not outlive the management HTTP budget.
            // Read-only early polls preserve the same challenge; the protocol still owns pacing.
            let device = inner.device.as_ref().ok_or(LoginError::Unavailable)?;
            if !device.poll_after().is_zero() {
                return Ok(());
            }
            let device = inner.device.take().ok_or(LoginError::Unavailable)?;
            inner.status = SessionStatus::Exchanging;
            device
        };
        let mut guard = ExchangeGuard {
            session: self,
            active: true,
        };
        let mut stop = self.cancel.subscribe();
        let result = tokio::select! {
            biased;
            _=wait_stopped(&mut stop)=>Err(LoginError::Stopped),
            _=tokio::time::sleep_until(self.deadline)=>Err(OAuthError::Expired.into()),
            result=self.client.poll_device_receipt(http,&mut device,self.constraint.as_ref())=>result.map_err(LoginError::from),
        };
        let mut inner = self.inner.lock();
        self.expire(&mut inner);
        if inner.status != SessionStatus::Exchanging {
            return Err(LoginError::Unavailable);
        }
        guard.active = false;
        match result {
            Ok(GrantPoll::Pending | GrantPoll::SlowDown) => {
                inner.device = Some(device);
                inner.status = SessionStatus::WaitingUser;
                self.expire(&mut inner);
                Ok(())
            }
            Ok(GrantPoll::Authorized(receipt)) if !receipt.is_expired() => {
                // Device flow has no code-flow nonce. Endpoint delivery is not a signed identity.
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
}
