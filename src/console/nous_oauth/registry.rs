//! Runtime-owned bounded sessions and accepted operations; no request owns the delivered successor.
use super::{Binding, LoginError, Session, SessionStatus, SessionView};
use crate::{
    error::GatewayError,
    protocol::nous_portal::OAuthClient,
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
        client: OAuthClient,
    ) -> Result<Arc<Session>, GatewayError> {
        let _creation = self
            .creations
            .try_read()
            .map_err(|_| super::login_error(LoginError::Stopped))?;
        let _slot = self
            .admissions
            .try_acquire()
            .map_err(|_| super::login_error(LoginError::Capacity))?;
        super::validate_target(state, &binding)?;
        {
            let mut sessions = self.sessions.lock();
            if *self.stop.borrow() {
                return Err(super::login_error(LoginError::Stopped));
            }
            sessions.retain(|_, s| s.retain());
            if sessions.len() >= 16 {
                return Err(super::login_error(LoginError::Capacity));
            }
        }
        let mut stop = self.stop.subscribe();
        let challenge = tokio::select! { biased;
            _ = wait_stopped(&mut stop) => return Err(super::login_error(LoginError::Stopped)),
            result = client.begin_device(state.upstream_client.client()) => result.map_err(|e| super::login_error(e.into()))?,
        };
        super::validate_target(state, &binding)?;
        let mut sessions = self.sessions.lock();
        if *self.stop.borrow() {
            return Err(super::login_error(LoginError::Stopped));
        }
        if sessions.len() >= 16 {
            return Err(super::login_error(LoginError::Capacity));
        }
        let session = Session::new(binding, client, challenge);
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
    pub(crate) fn start_poll(
        &self,
        state: Arc<AppState>,
        id: &str,
        binding: &Binding,
    ) -> Result<SessionView, GatewayError> {
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
        if session.view().status != SessionStatus::Exchanging {
            super::validate_target(&state, binding)?;
            if operations.len() >= 32 {
                return Err(super::login_error(LoginError::Capacity));
            }
            let challenge = session.take_challenge().map_err(super::login_error)?;
            spawn(
                &mut operations,
                super::polling::poll(state, session.clone(), challenge),
            );
        }
        Ok(session.view())
    }
    pub(crate) fn start_import(
        &self,
        state: Arc<AppState>,
        id: &str,
        binding: &Binding,
    ) -> Result<SessionView, GatewayError> {
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
        if operations.len() >= 32 && session.view().status == SessionStatus::Ready {
            return Err(super::login_error(LoginError::Capacity));
        }
        if let Some(operation) = session.begin_save(&state, binding)? {
            spawn(&mut operations, operation.run(state, session.clone()));
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
                    _ = shutdown.wait() => break,
                    _ = wait_stopped(&mut stop) => break,
                    _ = tick.tick() => {
                        let Some(registry) = weak.upgrade() else { break; };
                        registry.sessions.lock().retain(|_, s| s.retain());
                        reap(&mut registry.operations.lock());
                    }
                }
            }
            if let Some(registry) = weak.upgrade() {
                registry.shutdown();
            }
        }));
    }
    pub fn shutdown(&self) {
        let sessions = self.sessions.lock();
        self.stop.send_replace(true);
        self.admissions.close();
        for session in sessions.values() {
            session.cancel();
        }
    }
    pub async fn join(&self) {
        self.shutdown();
        let _creations = self.creations.write().await;
        let worker = self.worker.lock().take();
        if let Some(worker) = worker {
            report(worker.await);
        }
        let mut operations = std::mem::take(&mut *self.operations.lock());
        while let Some(result) = operations.join_next().await {
            report(result);
        }
    }
}
pub(super) async fn wait_stopped(stop: &mut watch::Receiver<bool>) {
    loop {
        if *stop.borrow_and_update() || stop.changed().await.is_err() {
            return;
        }
    }
}
fn reap(operations: &mut JoinSet<()>) {
    while let Some(result) = operations.try_join_next() {
        report(result);
    }
}
fn report(result: Result<(), tokio::task::JoinError>) {
    if result.is_err() {
        tracing::error!("Nous OAuth operation failed; result is unconfirmed");
    }
}
fn spawn(
    operations: &mut JoinSet<()>,
    operation: impl std::future::Future<Output = ()> + Send + 'static,
) {
    #[cfg(test)]
    let operation = {
        let origin = crate::protocol::nous_portal::TEST_ORIGIN
            .try_with(Clone::clone)
            .ok();
        async move {
            if let Some(origin) = origin {
                crate::protocol::nous_portal::TEST_ORIGIN
                    .scope(origin, operation)
                    .await;
            } else {
                operation.await;
            }
        }
    };
    operations.spawn(operation);
}
impl Drop for Registry {
    fn drop(&mut self) {
        for session in self.sessions.get_mut().values() {
            session.cancel();
        }
        if let Some(worker) = self.worker.get_mut().take() {
            worker.abort();
        }
        // JoinSet aborts remaining operations; normal runtime shutdown instead awaits join.
    }
}
impl std::fmt::Debug for Registry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("NousLoginRegistry { sessions: <redacted> }")
    }
}
