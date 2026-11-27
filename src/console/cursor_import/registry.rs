//! Bounded, runtime-owned single import per operator attempt; polling survives UI request completion.
use super::{error, save, Binding, Consent, Session, Start, View};
use crate::{
    error::GatewayError,
    protocol::cursor_auth::{self, Challenge},
    state::AppState,
};
use parking_lot::Mutex;
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::{task::JoinSet, time::Instant};

#[derive(Default)]
struct Inner {
    stopped: bool,
    sessions: HashMap<String, Arc<Session>>,
    tasks: JoinSet<()>,
}
#[derive(Default)]
pub(crate) struct Registry {
    inner: Mutex<Inner>,
}
impl std::fmt::Debug for Registry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("CursorImportRegistry { redacted }")
    }
}
impl Registry {
    pub fn create(
        &self,
        state: Arc<AppState>,
        owner: String,
        input: Start,
        consent: Consent,
    ) -> Result<View, GatewayError> {
        if uuid::Uuid::parse_str(&input.attempt_id).is_err()
            || [&input.provider_id, &input.expected_revision]
                .iter()
                .any(|s| s.is_empty() || s.len() > 256)
            || input
                .credential_id
                .as_ref()
                .is_some_and(|s| s.is_empty() || s.len() > 256)
        {
            return Err(error("cursor_import_invalid_input"));
        }
        let binding = Binding {
            owner,
            provider: input.provider_id,
            revision: input.expected_revision,
            credential: input.credential_id,
            email: input
                .expected_email
                .as_deref()
                .map(cursor_auth::normalize_email)
                .transpose()?,
        };
        let mut inner = self.inner.lock();
        if inner.stopped {
            return Err(error("cursor_import_stopped"));
        }
        while inner.tasks.try_join_next().is_some() {}
        inner.sessions.retain(|_, s| {
            Instant::now() < s.deadline + Duration::from_secs(300) || s.view().status == "importing"
        });
        if let Some(session) = inner.sessions.get(&input.attempt_id) {
            if session.binding != binding {
                return Err(error("cursor_import_attempt_conflict"));
            }
            return Ok(session.view());
        }
        if inner.sessions.len() >= 16 || inner.tasks.len() >= 16 {
            return Err(error("cursor_import_capacity"));
        }
        consent.check(&state)?;
        save::target(&state, &binding)?;
        if let Some(token) = &input.access_token {
            cursor_auth::require_fresh(cursor_auth::claims(token)?.exp)?;
        }
        let challenge = input
            .access_token
            .is_none()
            .then(Challenge::new)
            .transpose()?;
        let http = cursor_auth::client()?;
        let session = Session::new(
            input.attempt_id.clone(),
            binding,
            challenge.as_ref().map(|c| c.url.clone()),
        );
        inner.sessions.insert(input.attempt_id, session.clone());
        let view = session.view();
        // Own the guard before the task is polled, including runtime teardown immediately after spawn.
        let guard = Guard(session.clone());
        let operation = run(
            state,
            session,
            consent,
            http,
            challenge,
            input.access_token,
            guard,
        );
        #[cfg(test)]
        let operation = {
            let origin = cursor_auth::TEST_ORIGIN.try_with(Clone::clone).ok();
            async move {
                if let Some(origin) = origin {
                    cursor_auth::TEST_ORIGIN.scope(origin, operation).await;
                } else {
                    operation.await;
                }
            }
        };
        inner.tasks.spawn(operation);
        Ok(view)
    }
    pub fn get(&self, id: &str, owner: &str) -> Result<Arc<Session>, GatewayError> {
        self.inner
            .lock()
            .sessions
            .get(id)
            .filter(|s| s.binding.owner == owner)
            .cloned()
            .ok_or_else(|| error("cursor_import_session_unavailable"))
    }
    pub fn resume(
        &self,
        state: &AppState,
        id: &str,
        owner: &str,
        consent: Consent,
    ) -> Result<View, GatewayError> {
        consent.check(state)?;
        let session = self.get(id, owner)?;
        let mut view = session.inner.lock();
        if view.status == "awaiting_permission" {
            if Instant::now() >= session.deadline || *session.stop.borrow() {
                return Err(error("cursor_login_expired"));
            }
            // Fixed binding and revision: renewing management permission never starts another login.
            save::target(state, &session.binding)?;
            view.status = "verifying";
            view.error_code = None;
            session.renew.send_replace(Some(Arc::new(consent)));
        }
        Ok(view.clone())
    }
    pub fn close(&self) {
        let mut inner = self.inner.lock();
        inner.stopped = true;
        for session in inner.sessions.values() {
            session.cancel();
        }
    }
    pub async fn join(&self) {
        self.close();
        let mut tasks = std::mem::take(&mut self.inner.lock().tasks);
        while tasks.join_next().await.is_some() {}
    }
}
struct Guard(Arc<Session>);
impl Drop for Guard {
    fn drop(&mut self) {
        if matches!(
            self.0.view().status,
            "waiting_user" | "verifying" | "awaiting_permission" | "importing"
        ) {
            self.0.fail("cursor_import_interrupted");
        }
        // Terminalize before clearing: a concurrent resume can no longer repopulate the grant slot.
        self.0.renew.send_replace(None);
    }
}
async fn run(
    state: Arc<AppState>,
    session: Arc<Session>,
    consent: Consent,
    http: rquest::Client,
    challenge: Option<Challenge>,
    token: Option<String>,
    _guard: Guard,
) {
    let mut stop = session.stop.subscribe();
    if *stop.borrow() {
        return;
    }
    let receive = async {
        let token = if let Some(token) = token {
            token
        } else {
            let challenge = challenge.as_ref().unwrap();
            loop {
                if let Some(token) = cursor_auth::poll(&http, challenge).await? {
                    break token;
                }
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        };
        {
            let mut view = session.inner.lock();
            if *session.stop.borrow() {
                return Err(cursor_auth::AuthError("cursor_import_cancelled"));
            }
            view.auth_received = true;
            view.status = "verifying";
            view.authorization_url = None;
        }
        cursor_auth::verify(&http, token, session.binding.email.as_deref()).await
    };
    let material = tokio::select! { biased;
        _ = state.shutdown.wait() => { session.cancel(); return; },
        _ = stop.changed() => return,
        _ = tokio::time::sleep_until(session.deadline) => { session.fail("cursor_login_expired"); return; },
        result = receive => match result { Ok(material) => material, Err(e) => { session.fail(e.0); return; } }
    };
    session.inner.lock().account_matched = true;
    // The authorization challenge is no longer retained while saving.
    drop(challenge);
    super::completion::save_verified(&state, &session, consent, material).await;
}
