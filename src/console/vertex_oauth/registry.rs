//! Bounded per-runtime registry owns and joins listener/exchange and accepted save tasks.
use super::{session::wait_stopped, Binding, LoginError, Session, SessionView};
use crate::{
    protocol::vertex_oauth::{OAuthClient, VertexTarget},
    state::{AppState, GatewayShutdownHandle},
};
use parking_lot::Mutex;
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::{sync::watch, task::JoinSet};
pub struct Registry {
    sessions: Mutex<HashMap<String, Arc<Session>>>,
    stop: watch::Sender<bool>,
    worker: Mutex<Option<tokio::task::JoinHandle<()>>>,
    operations: Mutex<JoinSet<()>>,
}
impl Default for Registry {
    fn default() -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
            stop: watch::channel(false).0,
            worker: Mutex::new(None),
            operations: Mutex::new(JoinSet::new()),
        }
    }
}
impl Registry {
    pub(crate) async fn create(
        &self,
        state: &Arc<AppState>,
        binding: Binding,
        client: OAuthClient,
        target: VertexTarget,
        http: rquest::Client,
    ) -> Result<Arc<Session>, crate::error::GatewayError> {
        self.create_with_ttl(
            state,
            binding,
            client,
            target,
            http,
            Duration::from_secs(600),
        )
        .await
    }
    pub(super) async fn create_with_ttl(
        &self,
        state: &Arc<AppState>,
        binding: Binding,
        client: OAuthClient,
        target: VertexTarget,
        http: rquest::Client,
        ttl: Duration,
    ) -> Result<Arc<Session>, crate::error::GatewayError> {
        if !cfg!(feature = "line-google-agent-platform-official") {
            return Err(super::login_error(
                crate::protocol::vertex_oauth::OAuthError::FeatureDisabled.into(),
            ));
        }
        super::validate_target(state, &binding, &target)?;
        let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
            .await
            .map_err(|_| super::login_error(LoginError::Listener))?;
        let port = listener
            .local_addr()
            .map_err(|_| super::login_error(LoginError::Listener))?
            .port();
        // Admission and shutdown share this lock; no awaited bind happens while it is held.
        let mut sessions = self.sessions.lock();
        if *self.stop.borrow() {
            return Err(super::login_error(LoginError::Stopped));
        }
        sessions.retain(|_, s| s.retain());
        if sessions.len() >= 16 {
            return Err(super::login_error(LoginError::Capacity));
        }
        let session =
            Session::new(binding, client, target, port, ttl).map_err(super::login_error)?;
        let mut operations = self.operations.lock();
        reap(&mut operations);
        if operations.len() >= 32 {
            return Err(super::login_error(LoginError::Capacity));
        }
        let task = super::listener::serve(listener, session.clone(), Arc::downgrade(state), http);
        #[cfg(test)]
        let task = crate::protocol::vertex_oauth::transport_test_support::inherit_endpoint(task);
        operations.spawn(task);
        sessions.insert(session.id().into(), session.clone());
        Ok(session)
    }
    pub(crate) fn get_owned(&self, id: &str, owner: &str) -> Result<Arc<Session>, LoginError> {
        self.sessions
            .lock()
            .get(id)
            .filter(|s| s.binding.owner == owner && s.retain())
            .cloned()
            .ok_or(LoginError::Unavailable)
    }
    pub(crate) fn start_import(
        &self,
        state: Arc<AppState>,
        id: &str,
        binding: &Binding,
    ) -> Result<SessionView, crate::error::GatewayError> {
        let sessions = self.sessions.lock();
        if *self.stop.borrow() {
            return Err(super::login_error(LoginError::Stopped));
        }
        let session = sessions
            .get(id)
            .filter(|s| s.binding == *binding && s.retain())
            .ok_or_else(|| super::login_error(LoginError::Unavailable))?;
        let mut operations = self.operations.lock();
        reap(&mut operations);
        // Querying an already accepted save remains idempotent even at the task ceiling.
        if operations.len() >= 32
            && !matches!(
                session.view().status,
                super::SessionStatus::Importing
                    | super::SessionStatus::Succeeded
                    | super::SessionStatus::SaveUnconfirmed
            )
        {
            return Err(super::login_error(LoginError::Capacity));
        }
        if let Some(operation) = session.begin_save(&state, binding)? {
            operations.spawn(operation.run(state, session.clone()));
        }
        Ok(session.view())
    }
    pub fn spawn(self: &Arc<Self>, shutdown: GatewayShutdownHandle) {
        let mut worker = self.worker.lock();
        if worker.is_some() || *self.stop.borrow() {
            return;
        }
        let weak = Arc::downgrade(self);
        let mut stop = self.stop.subscribe();
        *worker = Some(tokio::spawn(async move {
            let mut tick = tokio::time::interval(Duration::from_secs(1));
            loop {
                tokio::select! { biased;
                    _=shutdown.wait()=>break,
                    _=wait_stopped(&mut stop)=>break,
                    _=tick.tick()=> { let Some(registry) = weak.upgrade() else { break; }; registry.sessions.lock().retain(|_, s| s.retain()); reap(&mut registry.operations.lock()); }
                }
            }
            if let Some(registry) = weak.upgrade() {
                registry.shutdown();
            }
        }));
    }
    pub fn shutdown(&self) {
        let mut sessions = self.sessions.lock();
        self.stop.send_replace(true);
        for session in sessions.values() {
            session.cancel();
        }
        sessions.clear();
    }
    pub async fn join(&self) {
        self.shutdown();
        let worker = self.worker.lock().take();
        if let Some(worker) = worker {
            if worker.await.is_err() {
                tracing::error!("Vertex OAuth expiry owner failed");
            }
        }
        let mut operations = std::mem::take(&mut *self.operations.lock());
        while let Some(result) = operations.join_next().await {
            if result.is_err() {
                tracing::error!("Vertex OAuth operation failed; save result may be unconfirmed");
            }
        }
    }
}
fn reap(operations: &mut JoinSet<()>) {
    while let Some(result) = operations.try_join_next() {
        if result.is_err() {
            tracing::error!("Vertex OAuth operation failed; save result may be unconfirmed");
        }
    }
}
impl Drop for Registry {
    fn drop(&mut self) {
        for session in self.sessions.get_mut().values() {
            session.cancel();
        }
        if let Some(worker) = self.worker.get_mut().take() {
            worker.abort();
        }
        // JoinSet drop aborts unjoined work; SaveGuard marks an admitted save unconfirmed.
    }
}
impl std::fmt::Debug for Registry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("VertexLoginRegistry { sessions: <redacted> }")
    }
}

#[cfg(test)]
mod tests;
