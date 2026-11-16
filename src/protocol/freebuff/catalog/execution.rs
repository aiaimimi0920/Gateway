//! Sequential catalog completions bound to the original durable session's live observation.
//! Not a full agent harness; moving the run prevents concurrent/replayed steps or guessed Drop FINISH.
mod decoded;
mod journal;
mod relay;
pub(super) mod root_turn;
pub(super) mod sequence;
mod step;
use super::{
    run_wire, BusinessError, BusinessRequests, CatalogError, ClientSurface, OwnedActiveSession,
    PreparedCompletion,
};
use bytes::Bytes;
use futures::Stream;
use sequence::RunAccounting;
use serde_json::Value;
use std::{
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

pub(crate) struct CatalogRun<'a> {
    pub(super) requests: &'a BusinessRequests<'a>,
    active: &'a OwnedActiveSession,
    run_id: String,
    client_id: String,
    accounting: RunAccounting,
    journal: journal::RunJournal<'a>,
    context: super::root_context::ContextState,
}
impl<'a> CatalogRun<'a> {
    #[cfg(test)]
    pub(crate) async fn start(
        requests: &'a BusinessRequests<'a>,
        active: &'a OwnedActiveSession,
        budget: Duration,
    ) -> Result<Self, BusinessError> {
        Self::start_inner(requests, active, budget, None).await
    }
    pub(crate) async fn start_recorded(
        requests: &'a BusinessRequests<'a>,
        active: &'a OwnedActiveSession,
        budget: Duration,
        recorder: Box<dyn super::run_journal::RunRecorder + 'a>,
    ) -> Result<Self, BusinessError> {
        Self::start_inner(requests, active, budget, Some(recorder)).await
    }
    async fn start_inner(
        requests: &'a BusinessRequests<'a>,
        active: &'a OwnedActiveSession,
        budget: Duration,
        recorder: Option<Box<dyn super::run_journal::RunRecorder + 'a>>,
    ) -> Result<Self, BusinessError> {
        check(requests, active)?;
        if budget.is_zero() {
            return Err(BusinessError::InvalidInput);
        }
        let deadline = tokio::time::Instant::now() + budget;
        let client_id = super::super::request::generate_client_session_id();
        let mut journal = journal::RunJournal::new(client_id.clone(), recorder);
        journal.save().await?;
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            return Err(BusinessError::Timeout);
        }
        check(requests, active)?;
        let run_id = run_wire::start(requests, remaining).await?;
        // Persist the real result even when liveness expired during START. It is not a new lease.
        journal.record.run_id = Some(run_id.clone());
        journal.record.phase = super::run_journal::RunPhase::Started;
        journal.save().await?;
        // A delayed START does not extend an expired observation. Its remote result stays unresolved.
        check(requests, active)?;
        Ok(Self {
            requests,
            active,
            run_id,
            client_id,
            accounting: RunAccounting::default(),
            journal,
            context: super::root_context::ContextState::default(),
        })
    }
    /// Only the owning step executor adds runtime identity; the host still supplies its full harness.
    fn prepare_body(
        &self,
        mut body: Value,
        limit: usize,
    ) -> Result<PreparedCompletion<'a>, BusinessError> {
        check(self.requests, self.active)?;
        if !body.is_object()
            || body.get("model").and_then(Value::as_str) != Some(self.requests.selected.handle)
            || body
                .get("codebuff_metadata")
                .is_some_and(|v| !v.is_object())
        {
            return Err(BusinessError::InvalidInput);
        }
        super::super::request::apply_runtime_metadata(
            &mut body,
            &self.run_id,
            "free",
            &self.client_id,
            Some(self.active.claim().instance_id()),
        );
        self.requests.prepare_completion(&body, limit)
    }
    /// Wire fixtures retain the split API; production uses execute_step so errors cannot lose the run.
    #[cfg(test)]
    pub(crate) fn prepare_completion(
        self,
        body: Value,
        limit: usize,
    ) -> Result<PreparedRunCompletion<'a>, BusinessError> {
        let prepared = self.prepare_body(body, limit)?;
        Ok(PreparedRunCompletion {
            run: self,
            prepared,
        })
    }
}
fn check(
    requests: &BusinessRequests<'_>,
    active: &OwnedActiveSession,
) -> Result<(), BusinessError> {
    if !cfg!(feature = "line-freebuff-web-reverse-api") {
        return Err(CatalogError::FeatureDisabled.into());
    }
    requests.check_claim(active.claim())?;
    if requests.binding.client != ClientSurface::Cli {
        return Err(CatalogError::Unsupported.into());
    }
    if requests.selected.key != active.model_key() {
        return Err(CatalogError::ScopeMismatch.into());
    }
    if !active.is_live() {
        return Err(BusinessError::SessionInactive);
    }
    Ok(())
}

#[cfg(test)]
pub(crate) struct PreparedRunCompletion<'a> {
    run: CatalogRun<'a>,
    prepared: PreparedCompletion<'a>,
}
#[cfg(test)]
impl<'a> PreparedRunCompletion<'a> {
    /// Recheck observation at the actual send, not only when the body was frozen.
    pub(crate) async fn send(
        mut self,
        budget: Duration,
    ) -> Result<CatalogCompletionResponse<'a>, BusinessError> {
        check(self.run.requests, self.run.active)?;
        self.run.journal.model_pending(&self.run.accounting).await?;
        check(self.run.requests, self.run.active)?;
        let response = self.prepared.send(budget).await?;
        Ok(CatalogCompletionResponse {
            run: self.run,
            response,
        })
    }
}
pub(crate) struct CatalogCompletionResponse<'a> {
    run: CatalogRun<'a>,
    response: rquest::Response,
}
impl<'a> CatalogCompletionResponse<'a> {
    pub(crate) fn status(&self) -> u16 {
        self.response.status().as_u16()
    }
    pub(crate) fn headers(&self) -> &rquest::header::HeaderMap {
        self.response.headers()
    }
    /// No raw response escape: the borrowed durable owner stays held through EOF, error or drop.
    /// Decoders still own their bounded collection/SSE parsing; HTTP 200 is not inference success.
    pub(crate) fn into_body(self) -> CatalogCompletionBody<'a> {
        CatalogCompletionBody {
            _run: self.run,
            stream: Box::pin(self.response.bytes_stream()),
        }
    }
}
impl std::fmt::Debug for CatalogRun<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("FreeBuffCatalogRun { identity: <redacted> }")
    }
}
#[cfg(test)]
impl std::fmt::Debug for PreparedRunCompletion<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("FreeBuffPreparedRunCompletion { body: <redacted> }")
    }
}
impl std::fmt::Debug for CatalogCompletionResponse<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FreeBuffCatalogCompletionResponse")
            .field("status", &self.status())
            .finish_non_exhaustive()
    }
}

pub(crate) struct CatalogCompletionBody<'a> {
    _run: CatalogRun<'a>,
    stream: Pin<Box<dyn Stream<Item = Result<Bytes, rquest::Error>> + Send>>,
}
impl Stream for CatalogCompletionBody<'_> {
    type Item = Result<Bytes, rquest::Error>;
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.stream.as_mut().poll_next(cx)
    }
}
impl std::fmt::Debug for CatalogCompletionBody<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("FreeBuffCatalogCompletionBody { body: <redacted> }")
    }
}
