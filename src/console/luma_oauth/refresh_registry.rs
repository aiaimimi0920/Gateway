//! Prepare is side-effect free; explicit import/resume uses the existing joined operation owner.
use super::{
    refresh::RefreshTarget, registry::reap, Binding, LoginError, Registry, Session, SessionView,
};
use crate::{
    console::{refresh_grants::GrantRequest, RouteConfigRedisRevision},
    error::GatewayError,
    state::AppState,
};
use std::sync::Arc;

impl Registry {
    pub(crate) fn prepare_refresh(
        &self,
        state: &AppState,
        owner: &str,
        provider: &str,
        credential: &str,
        revision: &str,
    ) -> Result<SessionView, GatewayError> {
        let _creation = self
            .creations
            .try_read()
            .map_err(|_| super::login_error(LoginError::Stopped))?;
        if *self.stop.borrow() {
            return Err(super::login_error(LoginError::Stopped));
        }
        let snapshot = state.route_config.snapshot();
        let context = snapshot
            .document()
            .providers
            .iter()
            .find(|p| p.id == provider)
            .and_then(|p| {
                p.credentials
                    .iter()
                    .find(|c| c.id.as_deref() == Some(credential))
            })
            .and_then(|c| c.luma_oauth.as_ref())
            .ok_or_else(|| super::login_error(LoginError::Unavailable))?;
        let binding = Binding::new(
            owner,
            provider,
            revision,
            context.client_id(),
            context.redirect_uri(),
        )
        .and_then(|b| b.with_credential(credential))
        .map_err(super::login_error)?
        .with_offline_access();
        let snapshot = super::validate_target(state, &binding)?;
        let revision =
            RouteConfigRedisRevision::new(snapshot.revision().clone(), snapshot.document().clone())
                .map_err(|_| super::login_error(LoginError::Unavailable))?;
        let request =
            GrantRequest::from_revision(&revision, provider, credential).map_err(|_| {
                GatewayError::bad_request("Luma offline authorization is required")
                    .with_code("luma_offline_required")
            })?;
        let mut sessions = self.sessions.lock();
        sessions.retain(|_, s| s.retain());
        if let Some(session) = sessions.values().find(|s| {
            s.refresh.is_some()
                && s.binding == binding
                && !matches!(
                    s.view().status,
                    super::SessionStatus::Cancelled | super::SessionStatus::Expired
                )
        }) {
            return Ok(session.view());
        }
        if sessions.len() >= 16 {
            return Err(super::login_error(LoginError::Capacity));
        }
        let session = Session::new_refresh(binding, RefreshTarget { revision, request });
        let view = session.view();
        sessions.insert(view.id.clone(), session);
        Ok(view)
    }

    pub(crate) fn resume_refresh(
        &self,
        state: Arc<AppState>,
        id: &str,
        owner: &str,
    ) -> Result<SessionView, GatewayError> {
        let _creation = self
            .creations
            .try_read()
            .map_err(|_| super::login_error(LoginError::Stopped))?;
        if *self.stop.borrow() || state.shutdown.is_requested() || state.lifecycle.is_draining() {
            return Err(super::login_error(LoginError::Stopped));
        }
        let session = self.get_owned(id, owner).map_err(super::login_error)?;
        let mut operations = self.operations.lock();
        reap(&mut operations);
        if operations.len() >= 32 {
            return Err(super::login_error(LoginError::Capacity));
        }
        if let Some(operation) = session.begin_refresh(true)? {
            let task = operation.run(state, session.clone());
            #[cfg(test)]
            let task = crate::protocol::luma_oauth::test_support::inherit_endpoint(task);
            operations.spawn(task);
        }
        Ok(session.view())
    }
}
