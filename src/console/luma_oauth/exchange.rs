//! One-shot exchange is cancellation-safe: wrong state is non-consuming, ambiguous code use is terminal.
use super::{
    session::{wait_stopped, Ready},
    LoginError, Session, SessionStatus,
};
use crate::protocol::luma_oauth::{AuthorizationContext, OAuthError};

impl Session {
    pub async fn complete(&self, http: &rquest::Client, callback: &str) -> Result<(), LoginError> {
        let grant = {
            let mut inner = self.inner.lock();
            self.expire(&mut inner);
            if inner.status != SessionStatus::WaitingUser {
                return Err(LoginError::Unavailable);
            }
            let auth = inner.auth.as_ref().ok_or(LoginError::Unavailable)?;
            if let Err(error) = auth.validate_callback(callback) {
                if matches!(error, OAuthError::AccessDenied | OAuthError::Rejected) {
                    self.finish(
                        &mut inner,
                        SessionStatus::Failed,
                        Some("luma_login_authorization_denied"),
                    );
                }
                return Err(error.into());
            }
            let auth = inner.auth.take().ok_or(LoginError::Unavailable)?;
            let grant = match auth.into_code_grant(callback) {
                Ok(grant) => grant,
                Err(error) => {
                    self.finish(
                        &mut inner,
                        SessionStatus::Expired,
                        Some("luma_login_expired"),
                    );
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
            _=tokio::time::sleep_until(self.deadline)=>Err(OAuthError::Expired.into()),
            result=grant.exchange(http)=>result.map_err(LoginError::from),
        }
        .and_then(|material| {
            Ok(Ready {
                context: AuthorizationContext::from_material(&material)?,
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
                self.finish(
                    &mut inner,
                    SessionStatus::Failed,
                    Some("luma_login_exchange_failed"),
                );
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
                self.session.finish(
                    &mut inner,
                    SessionStatus::Failed,
                    Some("luma_login_exchange_unconfirmed"),
                );
            }
        }
    }
}
