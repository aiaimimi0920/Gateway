//! Sequential foreground compaction and work share the original run, claim, cancellation and budget.
use super::super::{
    root_builder::{RootHostContext, RootStep},
    root_summary, BusinessError, RootHistory,
};
use super::{decoded::DecodedCatalogRun, CatalogRun};
use serde_json::Value;
use std::{future::Future, time::Duration};
use tokio::time::Instant;

pub(crate) struct RootTurn<'a> {
    pub(crate) context: RootHostContext<'a>,
    pub(crate) history: &'a mut RootHistory,
    pub(crate) tools: Vec<Value>,
    pub(crate) stream: bool,
}
impl RootTurn<'_> {
    fn step(&self, messages: Vec<Value>) -> RootStep<'_> {
        RootStep {
            context: RootHostContext {
                current_date: self.context.current_date,
                knowledge_files: self.context.knowledge_files,
                git_changes: self.context.git_changes,
                system_info: self.context.system_info,
            },
            messages,
            tools: self.tools.clone(),
            stream: self.stream,
        }
    }
}
impl<'a> CatalogRun<'a> {
    pub(crate) async fn execute_root_turn(
        mut self,
        turn: RootTurn<'_>,
        limit: usize,
        budget: Duration,
        cancelled: impl Future<Output = ()>,
    ) -> DecodedCatalogRun<'a> {
        self.context.begin();
        if budget.is_zero() || !turn.history.is_live_turn() {
            return DecodedCatalogRun::failed(self, BusinessError::InvalidInput, false).await;
        }
        let deadline = Instant::now() + budget.min(Duration::from_secs(300));
        tokio::pin!(cancelled);
        let stopped = tokio::select! { biased; _ = &mut cancelled => true, _ = std::future::ready(()) => false };
        if stopped {
            return DecodedCatalogRun::failed(self, BusinessError::Cancelled, false).await;
        }
        let (body, context) = match self
            .requests
            .build_root_step(turn.step(turn.history.messages().to_vec()), limit)
        {
            Ok(built) => built,
            Err(error) => return DecodedCatalogRun::failed(self, error, false).await,
        };
        let preview = self.context.preview(&context);
        let compact = match root_summary::due(preview, turn.history) {
            Ok(due) => due,
            Err(error) => return DecodedCatalogRun::failed(self, error, false).await,
        };
        let mut decoded = if compact {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return DecodedCatalogRun::failed(self, BusinessError::Timeout, false).await;
            }
            // Auxiliary usage belongs to the ledger, not the work-history calibration. Restore the
            // old receipt on failure so a failed summary cannot silently bypass the hard budget.
            let work_context = std::mem::take(&mut self.context);
            let mut summary = self
                .execute_step(
                    root_summary::request(body, preview.forecast.limit),
                    limit,
                    remaining,
                    &mut cancelled,
                )
                .await;
            summary.run.context = work_context;
            let text = match summary
                .completion
                .as_ref()
                .map_err(|e| *e)
                .and_then(root_summary::from_completion)
            {
                Ok(text) => text,
                Err(error) => {
                    summary.completion = Err(error);
                    return summary;
                }
            };
            let candidate = match turn.history.compacted(root_summary::memory(&text)) {
                Ok(candidate) => candidate,
                Err(error) => {
                    summary.completion = Err(error);
                    return summary;
                }
            };
            let (body, context) = match summary
                .run
                .requests
                .build_root_step(turn.step(candidate.messages().to_vec()), limit)
            {
                Ok(built) => built,
                Err(error) => {
                    summary.completion = Err(error);
                    return summary;
                }
            };
            // The replacement is a new history, sized without the old work receipt or summary usage.
            let forecast = super::super::root_context::ContextState::default()
                .preview(&context)
                .forecast;
            if forecast.tokens > forecast.limit {
                summary.completion = Err(BusinessError::ContextBudgetExceeded);
                return summary;
            }
            let stopped = tokio::select! { biased; _ = &mut cancelled => true, _ = std::future::ready(()) => false };
            if stopped {
                summary.completion = Err(BusinessError::Cancelled);
                return summary;
            }
            if deadline <= Instant::now() {
                summary.completion = Err(BusinessError::Timeout);
                return summary;
            }
            // Exclusive history borrow pins the covered prefix. Commit only a valid, bounded
            // summary and verbatim live user messages; no tool/host extension is executed.
            *turn.history = candidate;
            summary.run.context.reset();
            summary
                .run
                .execute_built_step(
                    || Ok((body, Some(context))),
                    limit,
                    deadline.saturating_duration_since(Instant::now()),
                    &mut cancelled,
                )
                .await
        } else {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return DecodedCatalogRun::failed(self, BusinessError::Timeout, false).await;
            }
            self.execute_built_step(
                || Ok((body, Some(context))),
                limit,
                remaining,
                &mut cancelled,
            )
            .await
        };
        if let Ok(body) = &decoded.completion {
            if let Err(error) = turn.history.record_completion(body) {
                decoded.completion = Err(error);
            }
        }
        decoded
    }
}
