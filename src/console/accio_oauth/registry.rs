//! Bounded per-runtime login owner, using the existing joined-save lifecycle, not a refresh scheduler.
use super::{session::wait_stopped, Binding, LoginError, Session, SessionView};
use crate::{
    protocol::accio_oauth::{LoginSite, OAuthClient},
    state::GatewayShutdownHandle,
};
use parking_lot::Mutex;
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::{sync::watch, task::JoinSet};

pub struct Registry {
    sessions: Mutex<HashMap<String, Arc<Session>>>,
    stop: watch::Sender<bool>,
    worker: Mutex<Option<tokio::task::JoinHandle<()>>>,
    imports: Mutex<JoinSet<()>>,
}
impl Default for Registry {
    fn default() -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
            stop: watch::channel(false).0,
            worker: Mutex::new(None),
            imports: Mutex::new(JoinSet::new()),
        }
    }
}
impl Registry {
    pub fn create(
        &self,
        binding: Binding,
        client: OAuthClient,
        site: LoginSite,
        port: u16,
        utdid: String,
    ) -> Result<Arc<Session>, LoginError> {
        self.create_with_ttl(binding, client, site, port, utdid, Duration::from_secs(600))
    }
    pub(super) fn create_with_ttl(
        &self,
        binding: Binding,
        client: OAuthClient,
        site: LoginSite,
        port: u16,
        utdid: String,
        ttl: Duration,
    ) -> Result<Arc<Session>, LoginError> {
        let mut sessions = self.sessions.lock();
        if *self.stop.borrow() {
            return Err(LoginError::Stopped);
        }
        sessions.retain(|_, s| s.retain());
        if sessions.len() >= 16 {
            return Err(LoginError::Capacity);
        }
        let session = Session::new(binding, client, site, port, utdid, ttl)?;
        sessions.insert(session.id().to_owned(), session.clone());
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
        state: Arc<crate::state::AppState>,
        id: &str,
        binding: &Binding,
    ) -> Result<SessionView, crate::error::GatewayError> {
        // Admission and shutdown use the same session lock. Accepted saves are not HTTP children.
        let sessions = self.sessions.lock();
        if *self.stop.borrow() {
            return Err(super::login_error(LoginError::Stopped));
        }
        let session = sessions
            .get(id)
            .filter(|s| s.binding == *binding && s.retain())
            .ok_or_else(|| super::login_error(LoginError::Unavailable))?;
        let mut imports = self.imports.lock();
        reap(&mut imports);
        if let Some(operation) = session.begin_save(&state, binding)? {
            imports.spawn(operation.run(state, session.clone()));
        }
        Ok(session.view())
    }
    pub fn spawn(self: &Arc<Self>, shutdown: GatewayShutdownHandle) {
        let mut handle = self.worker.lock();
        if handle.is_some() || *self.stop.borrow() {
            return;
        }
        let weak = Arc::downgrade(self);
        let mut stop = self.stop.subscribe();
        *handle = Some(tokio::spawn(async move {
            let mut tick = tokio::time::interval(Duration::from_secs(1));
            loop {
                tokio::select! { biased;
                    _=shutdown.wait()=>break,
                    _=wait_stopped(&mut stop)=>break,
                    _=tick.tick()=>{
                        let Some(registry)=weak.upgrade() else {break;};
                        registry.sessions.lock().retain(|_,s|s.retain());
                        reap(&mut registry.imports.lock());
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
        let task = self.worker.lock().take();
        if let Some(task) = task {
            if task.await.is_err() {
                tracing::error!("Accio OAuth expiry worker failed");
            }
        }
        let mut imports = std::mem::take(&mut *self.imports.lock());
        while let Some(result) = imports.join_next().await {
            if result.is_err() {
                tracing::error!("Accio OAuth save owner failed; result is unconfirmed");
            }
        }
    }
}
fn reap(imports: &mut JoinSet<()>) {
    while let Some(result) = imports.try_join_next() {
        if result.is_err() {
            tracing::error!("Accio OAuth save owner failed; result is unconfirmed");
        }
    }
}
impl Drop for Registry {
    fn drop(&mut self) {
        for session in self.sessions.get_mut().values() {
            session.cancel();
        }
        if let Some(task) = self.worker.get_mut().take() {
            task.abort();
        }
    }
}
impl std::fmt::Debug for Registry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("AccioLoginRegistry { sessions: <redacted> }")
    }
}
