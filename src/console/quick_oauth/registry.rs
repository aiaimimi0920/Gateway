//! Bounded session registry, loopback listener lifecycle, and graceful shutdown for `quick-oauth`.

use super::{session::wait_stopped, Binding, LoginError, Session, SessionView};
use crate::{
    protocol::quick_oauth::{transport::build_http_client, PendingLogin},
    state::{AppState, GatewayShutdownHandle},
};
use parking_lot::Mutex;
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::{
    net::TcpListener,
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
        super::validate_target(state, &binding)?;
        let http = build_http_client().map_err(|e| super::login_error(e.into()))?;
        let (pending, listener) = PendingLogin::start(http)
            .await
            .map_err(|e| super::login_error(e.into()))?;
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
        let session = Session::new(binding, pending);
        let task = drive_loopback(listener, session.clone(), state.shutdown.clone());
        #[cfg(test)]
        let task = inherit_test_origins(task);
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
                    _ = shutdown.wait() => break,
                    _ = wait_stopped(&mut stop) => break,
                    _ = tick.tick() => {
                        let Some(registry) = weak.upgrade() else { break };
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
        let _creations = self.creations.write().await;
        let worker = self.worker.lock().take();
        if let Some(worker) = worker {
            if worker.await.is_err() {
                tracing::error!("Amazon Quick OAuth expiry worker failed");
            }
        }
        let mut operations = std::mem::take(&mut *self.operations.lock());
        while let Some(result) = operations.join_next().await {
            if result.is_err() {
                tracing::error!("Amazon Quick OAuth operation failed");
            }
        }
    }
}

async fn drive_loopback(
    listener: TcpListener,
    session: Arc<Session>,
    shutdown: GatewayShutdownHandle,
) {
    let mut stop = session.stop.subscribe();
    let pending = session.pending().clone();
    let session_step = session.clone();
    tokio::select! {
        _ = shutdown.wait() => session.cancel(),
        _ = wait_stopped(&mut stop) => {}
        outcome = pending.run_loopback(listener, move |url| session_step.set_authorize_url(url)) => {
            match outcome {
                Ok(authorized) => session.complete_authorized(authorized),
                Err(err) => session.fail_oauth(err),
            }
        }
    }
}

#[cfg(test)]
fn inherit_test_origins<F>(fut: F) -> impl std::future::Future<Output = F::Output> + Send + 'static
where
    F: std::future::Future + Send + 'static,
    F::Output: Send + 'static,
{
    let oidc = crate::protocol::quick_oauth::TEST_OIDC_ORIGIN
        .try_with(|v| v.clone())
        .ok();
    let dp = crate::protocol::quick_oauth::TEST_DATA_PLANE_ORIGIN
        .try_with(|v| v.clone())
        .ok();
    async move {
        match (oidc, dp) {
            (Some(o), Some(d)) => {
                crate::protocol::quick_oauth::TEST_OIDC_ORIGIN
                    .scope(
                        o,
                        crate::protocol::quick_oauth::TEST_DATA_PLANE_ORIGIN.scope(d, fut),
                    )
                    .await
            }
            (Some(o), None) => {
                crate::protocol::quick_oauth::TEST_OIDC_ORIGIN
                    .scope(o, fut)
                    .await
            }
            (None, Some(d)) => {
                crate::protocol::quick_oauth::TEST_DATA_PLANE_ORIGIN
                    .scope(d, fut)
                    .await
            }
            (None, None) => fut.await,
        }
    }
}

fn reap(operations: &mut JoinSet<()>) {
    while let Some(result) = operations.try_join_next() {
        if result.is_err() {
            tracing::error!("Amazon Quick OAuth operation failed");
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
    }
}

impl std::fmt::Debug for Registry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("QuickLoginRegistry { sessions: <redacted> }")
    }
}
