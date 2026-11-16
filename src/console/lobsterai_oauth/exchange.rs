//! The registry owns the listener and exchange, with cancellation and target revalidation at each boundary.
use super::{session::wait_stopped, Session, SessionStatus};
use crate::{protocol::lobsterai::PendingLogin, state::AppState};
use std::sync::{Arc, Weak};
pub(super) async fn receive(pending: PendingLogin, session: Arc<Session>, state: Weak<AppState>) {
    let _guard = ExchangeGuard(session.clone());
    let mut stop = session.stop.subscribe();
    let result = tokio::select! { biased;
        _=wait_stopped(&mut stop)=>return,
        _=tokio::time::sleep_until(session.deadline)=>Err("lobsterai_login_expired"),
        result=async {
            let grant=pending.receive().await.map_err(|_| "lobsterai_login_callback_failed")?;
            let state=state.upgrade().ok_or("lobsterai_login_stopped")?;
            super::validate_target(&state,&session.binding).map_err(|_| "console_revision_conflict")?;
            { let mut inner=session.inner.lock(); session.expire(&mut inner);
                if inner.status!=SessionStatus::WaitingUser { return Err("lobsterai_login_unavailable"); }
                inner.status=SessionStatus::Exchanging;
            }
            let mut receipt=grant.exchange().await.map_err(|_| "lobsterai_login_exchange_failed")?;
            let material=receipt.admit().map_err(|_| "lobsterai_login_account_rejected")?.into_material();
            super::validate_target(&state,&session.binding).map_err(|_| "console_revision_conflict")?;
            Ok(material)
        }=>result,
    };
    let mut inner = session.inner.lock();
    session.expire(&mut inner);
    if !matches!(
        inner.status,
        SessionStatus::WaitingUser | SessionStatus::Exchanging
    ) {
        return;
    }
    match result {
        Ok(material) => {
            inner.material = Some(material);
            inner.status = SessionStatus::Ready;
        }
        Err(code) => {
            inner.status = SessionStatus::Failed;
            inner.error = Some(code);
        }
    }
    drop(inner);
}
struct ExchangeGuard(Arc<Session>);
impl Drop for ExchangeGuard {
    fn drop(&mut self) {
        let mut inner = self.0.inner.lock();
        if matches!(
            inner.status,
            SessionStatus::WaitingUser | SessionStatus::Exchanging
        ) {
            inner.status = SessionStatus::Failed;
            inner.material = None;
            inner.error = Some("lobsterai_login_exchange_unconfirmed");
        }
    }
}
