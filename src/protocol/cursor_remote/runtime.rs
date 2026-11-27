//! Bounded pure-Rust Cursor remote owner. Caller cancellation aborts the in-process task;
//! drain waits for the single execution slot, including detached settlement cleanup.
mod settings;
pub(crate) mod tools;
pub(crate) mod worker;
use crate::error::GatewayError;
use futures::future::BoxFuture;
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::{oneshot, Semaphore};
pub(crate) use tools::{Continuation, Publication, ToolTurn};

pub(crate) type Authorize =
    Box<dyn FnOnce() -> BoxFuture<'static, Result<(), GatewayError>> + Send>;
pub(crate) type Sent = Box<dyn FnOnce() + Send>;

#[cfg(test)]
pub(crate) type TestWorker = Arc<
    dyn for<'a> Fn(
            Value,
            &'a mut worker::WorkerHooks,
        ) -> BoxFuture<'a, Result<worker::WorkerOutput, GatewayError>>
        + Send
        + Sync,
>;

pub(crate) struct Runtime {
    settings: Result<Settings, &'static str>,
    slots: Arc<Semaphore>,
    closed: parking_lot::Mutex<bool>,
    tools: tools::Tools,
}

impl std::fmt::Debug for Runtime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Runtime")
            .field("available", &self.available())
            .finish()
    }
}

#[derive(Clone)]
struct Settings {
    proxy_port: u16,
    #[cfg(test)]
    worker: Option<TestWorker>,
}

impl Default for Runtime {
    fn default() -> Self {
        Self {
            settings: settings::resolve(),
            slots: Arc::new(Semaphore::new(1)),
            closed: parking_lot::Mutex::new(false),
            tools: tools::Tools::default(),
        }
    }
}

impl Runtime {
    pub(crate) fn available(&self) -> bool {
        self.ready().is_ok()
    }
    pub(crate) fn ready(&self) -> Result<(), GatewayError> {
        if *self.closed.lock() {
            return Err(super::error("draining"));
        }
        self.settings
            .as_ref()
            .map(|_| ())
            .map_err(|code| super::error(code))
    }
    pub(crate) fn close(&self) {
        *self.closed.lock() = true;
        self.tools.close();
    }
    pub(crate) async fn join(&self) {
        self.close();
        let _all = self.slots.acquire().await;
    }
    pub(crate) async fn execute(
        &self,
        mut input: Value,
        deadline: tokio::time::Instant,
        authorize: Authorize,
        sent: Sent,
        settled: tokio::sync::watch::Sender<bool>,
        uncertain: Arc<std::sync::atomic::AtomicBool>,
    ) -> Result<Completed, GatewayError> {
        let permit = {
            let closed = self.closed.lock();
            if *closed {
                return Err(super::error("draining"));
            }
            self.slots
                .clone()
                .try_acquire_owned()
                .map_err(|_| super::error("busy"))?
        };
        let settings = self.settings.clone().map_err(super::error)?;
        input["proxyPort"] = settings.proxy_port.into();
        let (cancel, cancelled) = oneshot::channel();
        let mut owner = Task {
            cancel: Some(cancel),
            task: tokio::spawn(async move {
                let _permit = permit;
                let _settled = Settled(settled);
                worker::run(
                    settings.proxy_port,
                    input,
                    #[cfg(test)]
                    settings.worker,
                    deadline,
                    authorize,
                    sent,
                    cancelled,
                    uncertain,
                    None,
                )
                .await
            }),
        };
        let result = (&mut owner.task)
            .await
            .map_err(|_| super::error("owner_failed"))?;
        owner.cancel.take();
        result
    }
    #[cfg(test)]
    pub(crate) fn fixture<F>(handler: F) -> Self
    where
        F: for<'a> Fn(
                Value,
                &'a mut worker::WorkerHooks,
            ) -> BoxFuture<'a, Result<worker::WorkerOutput, GatewayError>>
            + Send
            + Sync
            + 'static,
    {
        Self {
            settings: Ok(Settings {
                proxy_port: 1,
                worker: Some(Arc::new(handler)),
            }),
            slots: Arc::new(Semaphore::new(1)),
            closed: parking_lot::Mutex::new(false),
            tools: tools::Tools::default(),
        }
    }
    #[cfg(test)]
    pub(crate) async fn fixture_wait_idle(&self) {
        let _permit = self.slots.acquire().await.unwrap();
    }
    #[cfg(test)]
    pub(crate) fn unavailable(code: &'static str) -> Self {
        Self {
            settings: Err(code),
            slots: Arc::new(Semaphore::new(1)),
            closed: parking_lot::Mutex::new(false),
            tools: tools::Tools::default(),
        }
    }
}

struct Settled(tokio::sync::watch::Sender<bool>);
impl Drop for Settled {
    fn drop(&mut self) {
        let _ = self.0.send(true);
    }
}

pub(crate) async fn settle(receiver: Option<&tokio::sync::watch::Receiver<bool>>) {
    if let Some(receiver) = receiver {
        let mut receiver = receiver.clone();
        while !*receiver.borrow_and_update() {
            if receiver.changed().await.is_err() {
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod thinking_tests;
#[cfg(test)]
mod tool_custom_tests;
#[cfg(test)]
mod tool_incremental_tests;
#[cfg(test)]
mod tool_owner_tests;
#[cfg(test)]
mod tool_tests;

struct Task {
    cancel: Option<oneshot::Sender<()>>,
    task: tokio::task::JoinHandle<Result<Completed, GatewayError>>,
}
impl Drop for Task {
    fn drop(&mut self) {
        if let Some(cancel) = self.cancel.take() {
            let _ = cancel.send(());
        }
    }
}

#[derive(Debug)]
pub(crate) struct Completed {
    pub text: String,
    pub thinking: Option<super::thinking::PublicThinking>,
    pub selector: String,
    pub call: Option<ToolCall>,
    pub publication: Option<Publication>,
    pub usage: Option<crate::protocol::canonical::TokenUsage>,
}
#[derive(Debug)]
pub(crate) struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
    pub kind: super::tool_schema::ToolKind,
}

impl ToolCall {
    pub(super) fn history_item(&self) -> Value {
        let mut item = serde_json::json!({
            "type":self.kind.call_type(),"call_id":self.id,"name":self.name,
        });
        item[self.kind.input_field()] = serde_json::json!(self.arguments);
        item
    }

    pub(crate) fn into_canonical(self) -> crate::protocol::canonical::CanonicalToolCall {
        crate::protocol::canonical::CanonicalToolCall {
            id: Some(self.id),
            call_type: self.kind.canonical_type().into(),
            name: Some(self.name),
            arguments: Some(self.arguments),
            raw: Default::default(),
        }
    }
}
