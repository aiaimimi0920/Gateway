//! One cooperative step owner keeps the run through preparation, headers and body under one budget.
//! It never finishes on Drop, retries an unknown send, or converts missing accounting into zero.
use super::super::root_context::RootRequest;
use super::{
    check, decoded::DecodedCatalogRun, BusinessError, CatalogCompletionResponse, CatalogRun,
};
use serde_json::Value;
use std::{future::Future, time::Duration};
use tokio::time::Instant;

impl<'a> CatalogRun<'a> {
    /// Observe provisional native SSE frames; only the returned decoded owner can confirm EOF.
    pub(crate) async fn execute_step_observed<F, O>(
        self,
        body: Value,
        limit: usize,
        budget: Duration,
        cancelled: impl Future<Output = ()>,
        observe: O,
    ) -> DecodedCatalogRun<'a>
    where
        O: FnMut(Value) -> F,
        F: Future<Output = Result<(), BusinessError>>,
    {
        self.execute_built_step_observed(|| Ok((body, None)), limit, budget, cancelled, observe)
            .await
    }
    /// The trusted host supplies its final harness body. Cancellation is cooperative: await this
    /// future to get the unique owner back, then await explicit FINISH if its reported cost is known.
    /// The bounded synchronous serializer cannot be preempted; its elapsed time is charged before I/O.
    pub(crate) async fn execute_step(
        self,
        body: Value,
        limit: usize,
        budget: Duration,
        cancelled: impl Future<Output = ()>,
    ) -> DecodedCatalogRun<'a> {
        self.execute_built_step(|| Ok((body, None)), limit, budget, cancelled)
            .await
    }
    /// Default CLI prompt assembly only. A trusted host must provide actual tool implementations,
    /// compaction and a fresh bare native history; this checks a runtime budget but runs no tools.
    pub(crate) async fn execute_root_step(
        self,
        step: super::super::root_builder::RootStep<'_>,
        limit: usize,
        budget: Duration,
        cancelled: impl Future<Output = ()>,
    ) -> DecodedCatalogRun<'a> {
        let requests = self.requests;
        self.execute_built_step(
            || {
                requests
                    .build_root_step(step, limit)
                    .map(|(body, context)| (body, Some(context)))
            },
            limit,
            budget,
            cancelled,
        )
        .await
    }
    pub(super) async fn execute_built_step(
        self,
        build: impl FnOnce() -> Result<(Value, Option<RootRequest>), BusinessError>,
        limit: usize,
        budget: Duration,
        cancelled: impl Future<Output = ()>,
    ) -> DecodedCatalogRun<'a> {
        self.execute_built_step_observed(build, limit, budget, cancelled, |_| {
            std::future::ready(Ok(()))
        })
        .await
    }
    pub(super) async fn execute_built_step_observed<F, O>(
        mut self,
        build: impl FnOnce() -> Result<(Value, Option<RootRequest>), BusinessError>,
        limit: usize,
        budget: Duration,
        cancelled: impl Future<Output = ()>,
        observe: O,
    ) -> DecodedCatalogRun<'a>
    where
        O: FnMut(Value) -> F,
        F: Future<Output = Result<(), BusinessError>>,
    {
        self.context.begin();
        if budget.is_zero() {
            return DecodedCatalogRun::failed(self, BusinessError::InvalidInput, false).await;
        }
        let deadline = Instant::now() + budget.min(Duration::from_secs(300));
        tokio::pin!(cancelled);
        // Do not serialize or sign an already-cancelled request. The same future remains pinned
        // across all later phases; no detached cancellation task or independent timer is created.
        let stopped = tokio::select! {
            biased;
            _ = &mut cancelled => true,
            _ = std::future::ready(()) => false,
        };
        if stopped {
            return DecodedCatalogRun::failed(self, BusinessError::Cancelled, false).await;
        }
        let (body, context) = match build() {
            Ok(body) => body,
            Err(error) => return DecodedCatalogRun::failed(self, error, false).await,
        };
        if let Some(context) = context {
            if let Err(error) = self.context.prepare(context) {
                return DecodedCatalogRun::failed(self, error, false).await;
            }
        } else {
            // Arbitrary host harnesses do not carry root history witnesses or usage calibration.
            self.context.reset();
        }
        let prepared = match self.prepare_body(body, limit) {
            Ok(prepared) => prepared,
            Err(error) => return DecodedCatalogRun::failed(self, error, false).await,
        };
        if let Err(error) = check(self.requests, self.active) {
            return DecodedCatalogRun::failed(self, error, false).await;
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return DecodedCatalogRun::failed(self, BusinessError::Timeout, false).await;
        }
        if let Err(error) = self.journal.model_pending(&self.accounting).await {
            return DecodedCatalogRun::failed(self, error, false).await;
        }
        // The durable write can wait behind SQLite's writer lock; it cannot renew liveness.
        if let Err(error) = check(self.requests, self.active) {
            return DecodedCatalogRun::failed(self, error, false).await;
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return DecodedCatalogRun::failed(self, BusinessError::Timeout, false).await;
        }
        // Keep self OUTSIDE the cancellable send future. Once polled, failure means the send may
        // have reached the service, even if no response headers arrived. Never infer zero spend.
        let mut attempted = false;
        let response = {
            tokio::select! {
                biased;
                _ = &mut cancelled => Err(BusinessError::Cancelled),
                _ = tokio::time::sleep_until(deadline) => Err(BusinessError::Timeout),
                response = async {
                    attempted = true;
                    prepared.send(remaining).await
                } => response,
            }
        };
        let response = match response {
            Ok(response) => response,
            Err(error) => return DecodedCatalogRun::failed(self, error, attempted).await,
        };
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return DecodedCatalogRun::failed(self, BusinessError::Timeout, true).await;
        }
        CatalogCompletionResponse {
            run: self,
            response,
        }
        .decode_observed_until(remaining, &mut cancelled, observe)
        .await
    }
}
