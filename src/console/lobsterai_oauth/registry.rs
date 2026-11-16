//! Per-runtime bounds cover discovery admission, pending listeners and detached accepted saves.
use super::{session::wait_stopped, Binding, LoginError, Session, SessionView};
use crate::{
    protocol::lobsterai::PendingLogin,
    state::{AppState, GatewayShutdownHandle},
};
use parking_lot::Mutex;
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::{
    sync::{watch, Semaphore},
    task::JoinSet,
};

pub struct Registry {
    sessions: Mutex<HashMap<String, Arc<Session>>>,
    admissions: Semaphore,
    creations: tokio::sync::RwLock<()>,
    stop: watch::Sender<bool>,
    worker: Mutex<Option<tokio::task::JoinHandle<()>>>,
    operations: Mutex<JoinSet<()>>,
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
        }
    }
}
impl Registry {
    pub(crate) async fn create(
        &self,
        state: &Arc<AppState>,
        binding: Binding,
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
        let snapshot = super::validate_target(state, &binding)?;
        let client = super::target::client(&snapshot, &binding)?;
        let pending = PendingLogin::bind(client)
            .await
            .map_err(|e| super::login_error(e.into()))?;
        super::validate_target(state, &binding)?;
        // Shutdown shares this lock with final admission; no awaited I/O is performed under it.
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
        let session = Session::new(binding, &pending);
        let task = super::exchange::receive(pending, session.clone(), Arc::downgrade(state));
        #[cfg(test)]
        let task = crate::protocol::lobsterai::test_support::inherit_origin(task);
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
                super::SessionStatus::Importing
                    | super::SessionStatus::Succeeded
                    | super::SessionStatus::SaveUnconfirmed
            )
        {
            return Err(super::login_error(LoginError::Capacity));
        }
        if let Some(operation) = session.begin_save(&state, binding)? {
            let task = operation.run(state, session.clone());
            #[cfg(test)]
            let task = crate::protocol::lobsterai::test_support::inherit_origin(task);
            operations.spawn(task);
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
        self.admissions.close();
        for session in sessions.values() {
            session.cancel();
        }
        sessions.clear();
    }
    pub async fn join(&self) {
        self.shutdown();
        // Await caller-owned discovery futures too; their stop branch drops any bound listener.
        let _creations = self.creations.write().await;
        let worker = self.worker.lock().take();
        if let Some(worker) = worker {
            if worker.await.is_err() {
                tracing::error!("LobsterAI authorization expiry owner failed");
            }
        }
        let mut operations = std::mem::take(&mut *self.operations.lock());
        while let Some(result) = operations.join_next().await {
            if result.is_err() {
                tracing::error!(
                    "LobsterAI authorization operation failed; save result may be unconfirmed"
                );
            }
        }
    }
}
fn reap(operations: &mut JoinSet<()>) {
    while let Some(result) = operations.try_join_next() {
        if result.is_err() {
            tracing::error!(
                "LobsterAI authorization operation failed; save result may be unconfirmed"
            );
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
        // JoinSet aborts unjoined work; SaveGuard retains an unconfirmed receipt.
    }
}
impl std::fmt::Debug for Registry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("LobsterLoginRegistry { sessions: <redacted> }")
    }
}
