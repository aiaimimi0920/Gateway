//! Keep original run/claim borrow through EOF or cooperative cancellation, including partial costs.
use super::super::{completion_decode::Accounting, BusinessError};
use super::{CatalogCompletionResponse, CatalogRun};
use serde_json::Value;
use std::{future::Future, time::Duration};
#[path = "body_read.rs"]
mod body_read;

pub(crate) struct DecodedCatalogRun<'a> {
    pub(super) run: CatalogRun<'a>,
    pub(super) completion: Result<Value, BusinessError>,
    pub(super) accounting: Accounting,
}
impl<'a> CatalogCompletionResponse<'a> {
    /// Aggregate native JSON/SSE using the same incremental owner with a no-op frame observer.
    pub(crate) async fn decode(self, budget: Duration) -> DecodedCatalogRun<'a> {
        self.decode_until(budget, std::future::pending()).await
    }
    /// Cancellation is cooperative and returns the original owner. The host must await an explicit
    /// FINISH separately; dropping this future still sends nothing and cannot claim zero spend.
    pub(crate) async fn decode_until(
        self,
        budget: Duration,
        cancelled: impl Future<Output = ()>,
    ) -> DecodedCatalogRun<'a> {
        self.decode_observed_until(budget, cancelled, |_| std::future::ready(Ok(())))
            .await
    }
    /// Await the observer only after frame validation and durable accounting. Backpressure, observer
    /// errors, cancellation and deadline return this same run; callbacks never receive its authority.
    pub(crate) async fn decode_observed_until<F, O>(
        self,
        budget: Duration,
        cancelled: impl Future<Output = ()>,
        observe: O,
    ) -> DecodedCatalogRun<'a>
    where
        O: FnMut(Value) -> F,
        F: Future<Output = Result<(), BusinessError>>,
    {
        let status = self.status();
        let kind = self
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .split(';')
            .next()
            .unwrap_or("")
            .trim()
            .to_ascii_lowercase();
        let declared = self.response.content_length();
        let mut body = self.into_body();
        let mut accounting = Accounting::default();
        tokio::pin!(cancelled);
        let mut completion = if budget.is_zero() {
            Err(BusinessError::InvalidInput)
        } else {
            tokio::time::timeout(
                budget.min(Duration::from_secs(300)),
                body_read::read(
                    &mut body,
                    status,
                    &kind,
                    declared,
                    &mut accounting,
                    cancelled.as_mut(),
                    observe,
                ),
            )
            .await
            .unwrap_or(Err(BusinessError::Timeout))
        };
        let mut run = body._run;
        if let Err(error) = run.journal.observe(&run.accounting, &accounting).await {
            completion = Err(error);
        }
        // Moving the run through decode makes each step enter its cumulative ledger exactly once.
        run.accounting.record(&accounting);
        if let Err(error) = run
            .journal
            .stopped(&run.accounting, completion.as_ref().err().copied())
            .await
        {
            completion = Err(error);
        }
        // Only a fully validated native completion at real EOF can anchor a root context receipt.
        run.context.observe(completion.as_ref().ok());
        DecodedCatalogRun {
            run,
            completion,
            accounting,
        }
    }
}
impl<'a> DecodedCatalogRun<'a> {
    /// The executor knows whether the network future was polled. Possible sends have unknown cost;
    /// rejected preparation/cancellation before that boundary must not fabricate a model attempt.
    pub(super) async fn failed(
        mut run: CatalogRun<'a>,
        mut error: BusinessError,
        attempted: bool,
    ) -> Self {
        run.context.observe(None);
        let accounting = Accounting::default();
        if attempted {
            run.accounting.record(&accounting);
        }
        if let Err(failure) = run.journal.stopped(&run.accounting, Some(error)).await {
            error = failure;
        }
        Self {
            run,
            completion: Err(error),
            accounting,
        }
    }
    pub(crate) fn completion(&self) -> Result<&Value, BusinessError> {
        self.completion.as_ref().map_err(|e| *e)
    }
    /// A host's fallible output projection cannot change reported costs or acquire another run.
    /// Projection failure enters the existing explicit FINISH/journal failure path, not success.
    pub(crate) fn map_completion(
        mut self,
        project: impl FnOnce(Value) -> Result<Value, BusinessError>,
    ) -> Self {
        self.completion = self.completion.and_then(project);
        self
    }
    pub(crate) fn accounting(&self) -> &Accounting {
        &self.accounting
    }
    pub(crate) fn context_forecast(&self) -> Option<super::super::root_context::ContextForecast> {
        self.run.context.forecast()
    }
}
impl std::fmt::Debug for DecodedCatalogRun<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("FreeBuffDecodedCatalogRun { body: <redacted> }")
    }
}
