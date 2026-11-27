//! A single bounded live Run, parked only after a verified native MCP request.
//! The slot, H2 stream and cancellation owner survive HTTP tool-response boundaries.
use super::{worker, Authorize, Completed, Runtime, Sent};
use crate::{error::GatewayError, protocol::cursor_remote::tool_schema::Definition};
use serde_json::{json, Value};
use std::sync::{atomic::AtomicBool, Arc};
use tokio::sync::{mpsc, oneshot, watch};
use tokio::time::{Duration, Instant};
mod continuation;
pub(crate) use continuation::{Continuation, ToolTurn};

type Settlement = Arc<parking_lot::Mutex<watch::Sender<bool>>>;
#[derive(Debug, Default)]
pub(super) struct Tools {
    waiting: Arc<parking_lot::Mutex<Option<Session>>>,
    cancel: parking_lot::Mutex<Option<Arc<Cancel>>>,
}
#[derive(Debug)]
struct Cancel(parking_lot::Mutex<Option<oneshot::Sender<()>>>);
impl Cancel {
    fn stop(&self) {
        if let Some(tx) = self.0.lock().take() {
            let _ = tx.send(());
        }
    }
}
impl Tools {
    pub(super) fn close(&self) {
        if let Some(cancel) = self.cancel.lock().as_ref() {
            cancel.stop();
        }
        self.waiting.lock().take();
    }
}
#[derive(Debug)]
struct Session {
    id: uuid::Uuid,
    epoch: uuid::Uuid,
    published: bool,
    binding: String,
    signature: String,
    chat: bool,
    family: crate::protocol::canonical::ProtocolFamily,
    tools: Vec<Definition>,
    prefix: Vec<Value>,
    pending: String,
    pending_kind: super::super::tool_schema::ToolKind,
    expires: Instant,
    cancel: Arc<Cancel>,
    settlement: Settlement,
    events: mpsc::Receiver<Result<Completed, GatewayError>>,
    commands: mpsc::Sender<Round>,
}
/// HTTP owns this through body delivery; WS retains it for the live connection.
/// The epoch prevents an old response from cancelling a later pause of the same Run.
#[derive(Debug)]
pub(crate) struct Publication {
    epoch: Option<uuid::Uuid>,
    waiting: Arc<parking_lot::Mutex<Option<Session>>>,
}
impl Publication {
    pub(crate) fn commit(&mut self) -> bool {
        if !self.publish_on_connection() {
            return false;
        }
        self.epoch.take();
        true
    }
    /// A delivery preflight, not a reservation: expiry/worker exit can still race
    /// with the socket write, so publication must check the same state again.
    pub(crate) fn can_publish(&self) -> bool {
        self.waiting
            .lock()
            .as_ref()
            .is_some_and(|s| self.matches(s))
    }
    fn matches(&self, session: &Session) -> bool {
        self.epoch == Some(session.epoch)
            && session.expires > Instant::now()
            && !session.commands.is_closed()
    }
    /// Unlike HTTP commit, WS retains this epoch until the connection closes or
    /// replaces it with a newer pause. Dropping an old epoch cannot cancel the new one.
    pub(crate) fn publish_on_connection(&self) -> bool {
        let mut waiting = self.waiting.lock();
        if let Some(session) = waiting.as_mut().filter(|s| self.matches(s)) {
            session.published = true;
            return true;
        }
        false
    }
}
impl Drop for Publication {
    fn drop(&mut self) {
        let mut waiting = self.waiting.lock();
        if self
            .epoch
            .is_some_and(|epoch| waiting.as_ref().is_some_and(|s| s.epoch == epoch))
        {
            waiting.take();
        }
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        self.cancel.stop();
    }
}
pub(crate) struct Round {
    pub(crate) call_id: String,
    pub(crate) output: String,
    pub authorize: Authorize,
    pub sent: Sent,
    pub uncertain: Arc<AtomicBool>,
}
pub(crate) struct Bridge {
    pub events: mpsc::Sender<Result<Completed, GatewayError>>,
    pub commands: mpsc::Receiver<Round>,
}
impl Round {
    pub(super) fn wire(&self) -> Value {
        json!({"command":"continue","callId":self.call_id,"output":self.output})
    }
}
impl Runtime {
    pub(crate) async fn execute_tools(
        &self,
        binding: String,
        request: impl Into<ToolTurn>,
        mut input: Value,
        deadline: Instant,
        authorize: Authorize,
        sent: Sent,
        settled: watch::Sender<bool>,
        uncertain: Arc<AtomicBool>,
    ) -> Result<Completed, GatewayError> {
        self.ready()?;
        // Reject expired callers before consuming the legitimate published pause.
        if deadline <= Instant::now() {
            return Err(super::super::error("deadline"));
        }
        let request = request.into();
        let mut session = if let Some(output) = request.output() {
            let session = self.tools.resume(&binding, &request)?;
            *session.settlement.lock() = settled.clone();
            session
                .commands
                .try_send(Round {
                    call_id: output.call_id.clone(),
                    output: output.text.clone(),
                    authorize,
                    sent,
                    uncertain,
                })
                .map_err(|_| super::super::error("continuation_closed"))?;
            session
        } else {
            let ToolTurn::Full(request) = &request else {
                unreachable!("incremental turns always carry a tool result");
            };
            let _closed = self.closed.lock();
            if *_closed {
                return Err(super::super::error("draining"));
            }
            let permit = self
                .slots
                .clone()
                .try_acquire_owned()
                .map_err(|_| super::super::error("busy"))?;
            if self.tools.waiting.lock().is_some() {
                return Err(super::super::error("busy"));
            }
            let settings = self.settings.clone().map_err(super::super::error)?;
            input["proxyPort"] = settings.proxy_port.into();
            input["tools"] = json!(request
                .tools
                .iter()
                .map(|tool| &tool.wire)
                .collect::<Vec<_>>());
            let (cancel, cancelled) = oneshot::channel();
            let cancel = Arc::new(Cancel(parking_lot::Mutex::new(Some(cancel))));
            *self.tools.cancel.lock() = Some(cancel.clone());
            let (events, receiver) = mpsc::channel(1);
            let (commands, command_rx) = mpsc::channel(1);
            let settlement = Arc::new(parking_lot::Mutex::new(settled.clone()));
            let finish = settlement.clone();
            let expires = Instant::now() + Duration::from_secs(300);
            let id = uuid::Uuid::new_v4();
            let registry = self.tools.waiting.clone();
            tokio::spawn(async move {
                let _permit = permit;
                let result = worker::run(
                    settings.proxy_port,
                    input,
                    #[cfg(test)]
                    settings.worker,
                    expires,
                    authorize,
                    sent,
                    cancelled,
                    uncertain,
                    Some(Bridge {
                        events: events.clone(),
                        commands: command_rx,
                    }),
                )
                .await;
                // A parked failed/expired Run is not available for replay or a new claim.
                {
                    let mut waiting = registry.lock();
                    if waiting.as_ref().is_some_and(|s| s.id == id) {
                        waiting.take();
                    }
                }
                let _ = events.send(result).await;
                let _ = finish.lock().send(true);
            });
            Session {
                id,
                epoch: uuid::Uuid::new_v4(),
                published: false,
                binding,
                signature: request.signature.clone(),
                chat: request.chat,
                family: request.family,
                tools: request.tools.clone(),
                prefix: request.history.clone(),
                pending: String::new(),
                pending_kind: super::super::tool_schema::ToolKind::Function,
                expires,
                cancel,
                settlement,
                events: receiver,
                commands,
            }
        };
        let mut result = tokio::time::timeout_at(deadline, session.events.recv())
            .await
            .map_err(|_| super::super::error("deadline"))?
            .ok_or_else(|| super::super::error("continuation_closed"))??;
        if let Some(call) = &mut result.call {
            let definition = session
                .tools
                .iter()
                .find(|tool| tool.name == call.name)
                .ok_or_else(|| super::super::error("worker_protocol"))?;
            if result.text.len() > 65536 || call.arguments.len() > 65536 {
                return Err(super::super::error("worker_protocol"));
            }
            call.arguments = definition
                .kind
                .native_input(std::mem::take(&mut call.arguments))?;
            call.kind = definition.kind;
            if session.family == crate::protocol::canonical::ProtocolFamily::GeminiGenerateContent {
                let value = serde_json::from_str(&call.arguments)
                    .map_err(|_| super::super::error("worker_protocol"))?;
                call.arguments = super::super::gemini_request::object_text(&value)
                    .map_err(|_| super::super::error("worker_protocol"))?;
            }
            // These native protocols do not publish a compatible thinking field.
            // Never require clients to echo hidden or fabricated reasoning.
            if let Some(thinking) = result
                .thinking
                .as_ref()
                .filter(|_| session.family == crate::protocol::canonical::ProtocolFamily::OpenAi)
            {
                session.prefix.push(thinking.history_item(session.chat));
            }
            if !result.text.is_empty() {
                session
                    .prefix
                    .push(json!({"role":"assistant","content":result.text}));
            }
            session.prefix.push(call.history_item());
            // Do not publish a pause with no item slot left for its tool result.
            if session.prefix.len() >= 256
                || serde_json::to_vec(&session.prefix).map_or(usize::MAX, |v| v.len()) > 384 * 1024
            {
                return Err(super::super::error("history_limit"));
            }
            session.pending = call.id.clone();
            session.pending_kind = call.kind;
            session.epoch = uuid::Uuid::new_v4();
            session.published = false;
            let closed = self.closed.lock();
            if *closed {
                return Err(super::super::error("draining"));
            }
            let mut waiting = self.tools.waiting.lock();
            if session.commands.is_closed() {
                return Err(super::super::error("continuation_closed"));
            }
            result.publication = Some(Publication {
                epoch: Some(session.epoch),
                waiting: self.tools.waiting.clone(),
            });
            *waiting = Some(session);
            // This phase is settled by explicit transfer to the bounded live owner,
            // not by pretending the upstream Run has completed or the worker exited.
            let _ = settled.send(true);
        } else {
            let _ = settled.send(true);
        }
        Ok(result)
    }
}
