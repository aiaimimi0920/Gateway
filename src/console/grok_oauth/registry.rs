//! One bounded registry and one joined expiry worker per Gateway instance, never a global map.
use super::{session::wait_stopped, Binding, LoginError, Session, SessionView};
use crate::{
    protocol::grok_oauth::{OAuthClient, PrincipalConstraint},
    state::GatewayShutdownHandle,
};
use parking_lot::Mutex;
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::sync::watch;
use tokio::task::JoinSet;

pub struct Registry {
    sessions: Mutex<HashMap<String, Arc<Session>>>,
    stop: watch::Sender<bool>,
    worker: Mutex<Option<tokio::task::JoinHandle<()>>>,
    imports: Mutex<JoinSet<()>>,
    listeners: Mutex<JoinSet<()>>,
}
impl Default for Registry {
    fn default() -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
            stop: watch::channel(false).0,
            worker: Mutex::new(None),
            imports: Mutex::new(JoinSet::new()),
            listeners: Mutex::new(JoinSet::new()),
        }
    }
}
impl Registry {
    pub(crate) fn create_loopback(
        &self,
        state: &Arc<crate::state::AppState>,
        binding: Binding,
        client: OAuthClient,
        redirect: &str,
        constraint: Option<PrincipalConstraint>,
        http: rquest::Client,
    ) -> Result<Arc<Session>, LoginError> {
        // Admission, socket binding and task registration share shutdown's lock; no awaited I/O.
        let mut sessions = self.sessions.lock();
        if *self.stop.borrow() {
            return Err(LoginError::Stopped);
        }
        sessions.retain(|_, s| s.retain());
        let mut listeners = self.listeners.lock();
        reap_listeners(&mut listeners);
        if sessions.len() >= 16 || listeners.len() >= 16 {
            return Err(LoginError::Capacity);
        }
        let session = Session::new(
            binding,
            client,
            redirect,
            constraint,
            Duration::from_secs(600),
        )?;
        let redirect = session
            .inner
            .lock()
            .auth
            .as_ref()
            .expect("new code session owns authorization")
            .redirect_uri()
            .to_owned();
        let listener = super::listener::bind(&redirect)?;
        session.inner.lock().automatic_callback = true;
        listeners.spawn(super::listener::serve(
            listener,
            session.clone(),
            Arc::downgrade(state),
            http,
            redirect,
        ));
        sessions.insert(session.id().to_owned(), session.clone());
        Ok(session)
    }
    pub fn create(
        &self,
        binding: Binding,
        client: OAuthClient,
        redirect: &str,
        constraint: Option<PrincipalConstraint>,
    ) -> Result<Arc<Session>, LoginError> {
        self.create_with_ttl(
            binding,
            client,
            redirect,
            constraint,
            Duration::from_secs(600),
        )
    }
    pub async fn create_device(
        &self,
        binding: Binding,
        client: OAuthClient,
        constraint: Option<PrincipalConstraint>,
        http: &rquest::Client,
    ) -> Result<Arc<Session>, LoginError> {
        let session = {
            let mut sessions = self.sessions.lock();
            if *self.stop.borrow() {
                return Err(LoginError::Stopped);
            }
            sessions.retain(|_, s| s.retain());
            if sessions.len() >= 16 {
                return Err(LoginError::Capacity);
            }
            // Reserve admission before network I/O; no detached poll task or unbounded grants.
            let session =
                Session::build(binding, client, constraint, Duration::from_secs(600), None);
            sessions.insert(session.id().to_owned(), session.clone());
            session
        };
        session.begin_device(http).await?;
        Ok(session)
    }
    fn create_with_ttl(
        &self,
        binding: Binding,
        client: OAuthClient,
        redirect: &str,
        constraint: Option<PrincipalConstraint>,
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
        let session = Session::new(binding, client, redirect, constraint, ttl)?;
        sessions.insert(session.id().to_owned(), session.clone());
        Ok(session)
    }
    pub fn get(&self, id: &str, binding: &Binding) -> Result<Arc<Session>, LoginError> {
        let sessions = self.sessions.lock();
        sessions
            .get(id)
            .filter(|s| s.binding == *binding && s.retain())
            .cloned()
            .ok_or(LoginError::Unavailable)
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
                tokio::select! {
                    biased;
                    _=shutdown.wait()=>break,
                    _=wait_stopped(&mut stop)=>break,
                    _=tick.tick()=>{
                        let Some(registry)=weak.upgrade() else {break;};
                        registry.sessions.lock().retain(|_,s|s.retain());
                        reap(&mut registry.imports.lock());
                        reap_listeners(&mut registry.listeners.lock());
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
                tracing::error!("Grok OAuth expiry worker failed");
            }
        }
        let mut imports = std::mem::take(&mut *self.imports.lock());
        let mut listeners = std::mem::take(&mut *self.listeners.lock());
        while let Some(result) = listeners.join_next().await {
            if result.is_err() {
                tracing::error!("Grok OAuth callback listener failed");
            }
        }
        while let Some(result) = imports.join_next().await {
            if result.is_err() {
                tracing::error!("Grok OAuth save owner failed; result is unconfirmed");
            }
        }
    }
    #[cfg(test)]
    pub(crate) fn count(&self) -> usize {
        self.sessions.lock().len()
    }
    #[cfg(test)]
    pub(super) fn short_session(
        &self,
        binding: Binding,
        client: OAuthClient,
        ttl: Duration,
    ) -> Arc<Session> {
        self.create_with_ttl(
            binding,
            client,
            "http://127.0.0.1:34567/callback",
            None,
            ttl,
        )
        .unwrap()
    }
}
fn reap(imports: &mut JoinSet<()>) {
    while let Some(result) = imports.try_join_next() {
        if result.is_err() {
            tracing::error!("Grok OAuth save owner failed; result is unconfirmed");
        }
    }
}
fn reap_listeners(listeners: &mut JoinSet<()>) {
    while let Some(result) = listeners.try_join_next() {
        if result.is_err() {
            tracing::error!("Grok OAuth callback listener failed");
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
        f.write_str("GrokLoginRegistry { sessions: <redacted> }")
    }
}
