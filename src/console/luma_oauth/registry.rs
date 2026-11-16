//! Runtime-owned bounded sessions/listeners/saves; shutdown closes admission and joins accepted work.
use super::{session::wait_stopped, Binding, LoginError, Session, SessionStatus, SessionView};
use crate::{
    protocol::luma_oauth::Authorization,
    state::{AppState, GatewayShutdownHandle},
};
use parking_lot::Mutex;
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::{
    sync::{watch, Semaphore},
    task::JoinSet,
};

pub struct Registry {
    pub(super) sessions: Mutex<HashMap<String, Arc<Session>>>,
    pub(super) admissions: Semaphore,
    pub(super) creations: tokio::sync::RwLock<()>,
    pub(super) stop: watch::Sender<bool>,
    worker: Mutex<Option<tokio::task::JoinHandle<()>>>,
    pub(super) operations: Mutex<JoinSet<()>>,
    pub(super) registrations: Mutex<HashMap<String, Arc<super::registration::Registration>>>,
}
impl Default for Registry {
    fn default() -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
            admissions: Semaphore::new(16),
            creations: tokio::sync::RwLock::new(()),
            stop: watch::channel(false).0,
            worker: Mutex::new(None),
            operations: Mutex::new(JoinSet::new()),
            registrations: Mutex::new(HashMap::new()),
        }
    }
}
impl Registry {
    pub(crate) async fn create(
        &self,
        state: &Arc<AppState>,
        binding: Binding,
        http: rquest::Client,
    ) -> Result<Arc<Session>, crate::error::GatewayError> {
        self.create_with_ttl(state, binding, http, Duration::from_secs(1200))
            .await
    }
    pub(super) async fn create_with_ttl(
        &self,
        state: &Arc<AppState>,
        binding: Binding,
        http: rquest::Client,
        ttl: Duration,
    ) -> Result<Arc<Session>, crate::error::GatewayError> {
        let _creation = self
            .creations
            .try_read()
            .map_err(|_| super::login_error(LoginError::Stopped))?;
        if *self.stop.borrow() {
            return Err(super::login_error(LoginError::Stopped));
        }
        let _admission = self
            .admissions
            .try_acquire()
            .map_err(|_| super::login_error(LoginError::Capacity))?;
        super::validate_target(state, &binding)?;
        let mut auth = Authorization::new(&binding.client, &binding.redirect)
            .map_err(|e| super::login_error(e.into()))?;
        if binding.offline_access {
            // Old receipts default to code-only. Neither changing owner nor redirect upgrades a client.
            if super::registration_storage::load(state)
                .await?
                .iter()
                .any(|r| {
                    r.client_id.as_deref() == Some(binding.client.as_str())
                        && (!r.offline_access || r.redirect_uri != binding.redirect)
                })
            {
                return Err(crate::error::GatewayError::bad_request(
                    "A refresh-capable registered Luma client is required",
                )
                .with_code("luma_offline_client_required"));
            }
            auth = auth.with_offline_access();
        }
        let port = Authorization::callback_port(&binding.redirect)
            .map_err(|e| super::login_error(e.into()))?;
        {
            let mut sessions = self.sessions.lock();
            sessions.retain(|_, s| s.retain());
            if sessions.len() >= 16 {
                return Err(super::login_error(LoginError::Capacity));
            }
        }
        // Registration fixes this exact port; do not pick a different redirect on bind failure.
        let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port))
            .await
            .map_err(|_| super::login_error(LoginError::Listener))?;
        super::validate_target(state, &binding)?;
        let mut sessions = self.sessions.lock();
        if *self.stop.borrow() {
            return Err(super::login_error(LoginError::Stopped));
        }
        sessions.retain(|_, s| s.retain());
        let mut operations = self.operations.lock();
        reap(&mut operations);
        if sessions.len() >= 16 || operations.len() >= 32 {
            return Err(super::login_error(LoginError::Capacity));
        }
        let session = Session::new(binding, auth, ttl);
        let task = super::listener::serve(listener, session.clone(), Arc::downgrade(state), http);
        #[cfg(test)]
        let task = crate::protocol::luma_oauth::test_support::inherit_endpoint(task);
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
        if operations.len() >= 32
            && !matches!(
                session.view().status,
                SessionStatus::Importing
                    | SessionStatus::Succeeded
                    | SessionStatus::SaveUnconfirmed
            )
        {
            return Err(super::login_error(LoginError::Capacity));
        }
        if session.refresh.is_some() {
            if let Some(operation) = session.begin_refresh(false)? {
                let task = operation.run(state, session.clone());
                #[cfg(test)]
                let task = crate::protocol::luma_oauth::test_support::inherit_endpoint(task);
                operations.spawn(task);
            }
        } else if let Some(operation) = session.begin_save(&state, binding)? {
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
                    _=tick.tick()=>{let Some(registry)=weak.upgrade() else {break;}; registry.sessions.lock().retain(|_,s|s.retain());
                        for receipt in registry.registrations.lock().values() { receipt.view(); }
                        reap(&mut registry.operations.lock());}
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
        self.admissions.close();
        for receipt in self.registrations.lock().values() {
            receipt.cancel_prepared();
        }
        for session in sessions.values() {
            session.cancel();
        }
        sessions.clear();
    }
    pub async fn join(&self) {
        self.shutdown();
        let _creations = self.creations.write().await;
        let worker = self.worker.lock().take();
        if let Some(worker) = worker {
            if worker.await.is_err() {
                tracing::error!("Luma OAuth expiry owner failed");
            }
        }
        let mut operations = std::mem::take(&mut *self.operations.lock());
        while let Some(result) = operations.join_next().await {
            if result.is_err() {
                tracing::error!("Luma OAuth operation failed; save result may be unconfirmed");
            }
        }
    }
}
pub(super) fn reap(operations: &mut JoinSet<()>) {
    while let Some(result) = operations.try_join_next() {
        if result.is_err() {
            tracing::error!("Luma OAuth operation failed; save result may be unconfirmed");
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
        // JoinSet aborts unfinished operations; their guards record an unconfirmed outcome.
    }
}
impl std::fmt::Debug for Registry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("LumaLoginRegistry { sessions: <redacted> }")
    }
}
