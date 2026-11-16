//! Caller-owned public conversation, not a CLI turn: no prompt injection, auto-summary or tool loop.
use super::super::RelayStep;
use super::{decoded::DecodedCatalogRun, CatalogRun};
use std::{future::Future, time::Duration};
impl<'a> CatalogRun<'a> {
    pub(crate) async fn execute_relay_step(
        self,
        step: RelayStep,
        budget: Duration,
        cancelled: impl Future<Output = ()>,
    ) -> DecodedCatalogRun<'a> {
        self.execute_relay_step_observed(step, budget, cancelled, |_| std::future::ready(Ok(())))
            .await
    }
    pub(crate) async fn execute_relay_step_observed<F, O>(
        self,
        step: RelayStep,
        budget: Duration,
        cancelled: impl Future<Output = ()>,
        observe: O,
    ) -> DecodedCatalogRun<'a>
    where
        O: FnMut(serde_json::Value) -> F,
        F: Future<Output = Result<(), super::BusinessError>>,
    {
        let limit = step
            .intent()
            .len()
            .saturating_add(16384)
            .min(50 * 1024 * 1024);
        self.execute_built_step_observed(
            || Ok((step.body, Some(step.context))),
            limit,
            budget,
            cancelled,
            observe,
        )
        .await
    }
}
