//! Non-owning continuation capabilities and atomic, bounded pause admission.
use super::{Publication, Session, Tools};
use crate::{
    error::GatewayError,
    protocol::cursor_remote::{
        error,
        incremental::IncrementalRequest,
        text_output,
        tool_request::{ToolOutput, ToolRequest},
    },
};
use std::sync::{Arc, Weak};
use tokio::time::Instant;

#[derive(Clone, Debug)]
pub(crate) struct Continuation {
    epoch: uuid::Uuid,
    response_id: String,
    waiting: Weak<parking_lot::Mutex<Option<Session>>>,
}
impl Continuation {
    pub(crate) fn references(&self, response_id: &str) -> bool {
        self.response_id == response_id
    }
}
impl Publication {
    /// Issued only from a published epoch. Clones do not own cancellation or
    /// keep a worker alive; the connection retains the sole publication owner.
    pub(crate) fn continuation(&self, response_id: &str) -> Option<Continuation> {
        let waiting = self.waiting.lock();
        let session = waiting.as_ref()?;
        if !self.matches(session) || !session.published {
            return None;
        }
        Some(Continuation {
            epoch: session.epoch,
            response_id: response_id.into(),
            waiting: Arc::downgrade(&self.waiting),
        })
    }
}

pub(crate) enum ToolTurn {
    Full(ToolRequest),
    Incremental(IncrementalRequest),
}
impl From<ToolRequest> for ToolTurn {
    fn from(request: ToolRequest) -> Self {
        Self::Full(request)
    }
}
impl From<IncrementalRequest> for ToolTurn {
    fn from(request: IncrementalRequest) -> Self {
        Self::Incremental(request)
    }
}
impl ToolTurn {
    pub(super) fn output(&self) -> Option<&ToolOutput> {
        match self {
            Self::Full(request) => request.output.as_ref(),
            Self::Incremental(request) => Some(&request.output),
        }
    }
}

impl Tools {
    pub(super) fn resume(
        &self,
        binding: &str,
        request: &ToolTurn,
    ) -> Result<Session, GatewayError> {
        let output = request.output().expect("resume requires a tool result");
        text_output::check_ipc(&output.call_id, &output.text)?;
        let item = output.history_item();
        let mut waiting = self.waiting.lock();
        let session = waiting
            .as_ref()
            .ok_or_else(|| error("continuation_missing"))?;
        let request_matches = match request {
            ToolTurn::Full(request) => {
                session.chat == request.chat
                    && session.family == request.family
                    && session.signature == request.signature
                    && request
                        .history
                        .split_last()
                        .is_some_and(|(_, prefix)| prefix == session.prefix.as_slice())
            }
            ToolTurn::Incremental(request) => {
                request.continuation.epoch == session.epoch
                    && request
                        .continuation
                        .waiting
                        .upgrade()
                        .is_some_and(|registry| Arc::ptr_eq(&registry, &self.waiting))
            }
        };
        if !request_matches
            || !session.published
            || session.binding != binding
            || session.pending != output.call_id
            || session.pending_kind != output.kind
            || session.expires <= Instant::now()
            || session.commands.is_closed()
        {
            return Err(error("continuation_mismatch"));
        }
        // Check the prospective prefix before take/authorization/send. A bad
        // request leaves the legitimate pause intact, including its original TTL.
        let bytes = serde_json::to_vec(&session.prefix)
            .map_err(|_| error("history_limit"))?
            .len();
        let added = serde_json::to_vec(&item)
            .map_err(|_| error("history_limit"))?
            .len()
            + 1;
        if session.prefix.len() >= 256 || bytes.saturating_add(added) > 384 * 1024 {
            return Err(error("history_limit"));
        }
        let mut session = waiting.take().unwrap();
        session.prefix.push(item);
        Ok(session)
    }
}

#[cfg(test)]
mod tests;
