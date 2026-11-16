//! One runtime owns capacity, TTL cleanup, approval steps and accepted durable-save tasks.
use super::{session::wait_stopped, Binding, LoginError, Session, SessionView};
use crate::{
    protocol::freebuff_login::{LoginError as WireError, LoginTransport},
    state::GatewayShutdownHandle,
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
    pub fn start(
        &self,
        binding: Binding,
        http: LoginTransport,
        fingerprint: String,
    ) -> Result<Arc<Session>, LoginError> {
        if !cfg!(feature = "line-freebuff-web-reverse-api") {
            return Err(WireError::FeatureDisabled.into());
        }
        if !crate::protocol::freebuff_login::atom(&fingerprint, 512) {
            return Err(WireError::InvalidInput.into());
        }
        let mut sessions = self.sessions.lock();
        if *self.stop.borrow() {
            return Err(LoginError::Stopped);
        }
        sessions.retain(|_, s| s.retain());
        if sessions.len() >= 16 {
            return Err(LoginError::Capacity);
        }
        let session = Session::new(binding, fingerprint, Duration::from_secs(600));
        let mut operations = self.operations.lock();
        reap(&mut operations);
        spawn(&mut operations, session.initialize(http));
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
        id: &str,
        binding: &Binding,
        http: LoginTransport,
    ) -> Result<SessionView, LoginError> {
        let sessions = self.sessions.lock();
        if *self.stop.borrow() {
            return Err(LoginError::Stopped);
        }
        let session = sessions
            .get(id)
            .filter(|s| s.binding == *binding && s.retain())
            .ok_or(LoginError::Unavailable)?;
        let mut operations = self.operations.lock();
        reap(&mut operations);
        if session.view().status != super::SessionStatus::Polling {
            spawn(&mut operations, session.begin_poll(http)?);
        }
        Ok(session.view())
    }
    pub(crate) fn start_import(
        &self,
        state: Arc<crate::state::AppState>,
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
                    _=shutdown.wait()=>break,
                    _=wait_stopped(&mut stop)=>break,
                    _=tick.tick()=>{
                        let Some(registry)=weak.upgrade() else { break; };
                        registry.sessions.lock().retain(|_,s|s.retain());
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
            report(worker.await);
        }
        let mut operations = std::mem::take(&mut *self.operations.lock());
        while let Some(result) = operations.join_next().await {
            report(result);
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
        tracing::error!("FreeBuff login operation owner failed; login/save result is unconfirmed");
    }
}
fn spawn(
    operations: &mut JoinSet<()>,
    operation: impl std::future::Future<Output = ()> + Send + 'static,
) {
    // Tokio task-locals do not propagate through spawn. Production has no configurable origin.
    #[cfg(all(test, feature = "line-freebuff-web-reverse-api"))]
    let operation = {
        let origin = crate::protocol::freebuff_login::TEST_ORIGIN
            .try_with(Clone::clone)
            .ok();
        async move {
            if let Some(origin) = origin {
                crate::protocol::freebuff_login::TEST_ORIGIN
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
        // JoinSet aborts operations on drop; each operation closes its own admission through a guard.
    }
}
impl std::fmt::Debug for Registry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("FreeBuffLoginRegistry { sessions: <redacted> }")
    }
}
