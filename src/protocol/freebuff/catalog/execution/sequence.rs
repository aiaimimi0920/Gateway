//! O(1) sequential-model accounting. No tool/child charges are inferred or read from caller metadata.
use super::super::{
    completion_decode::{Accounting, MAX_SAFE_INTEGER},
    finish_wire, BusinessError,
};
use super::{decoded::DecodedCatalogRun, CatalogRun};
use serde_json::Value;
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct RunTotals {
    pub(crate) total_steps: u64,
    pub(crate) direct_credits: u64,
    pub(crate) total_credits: u64,
}
#[derive(Clone)]
pub(super) struct RunAccounting {
    steps: Option<u64>,
    credits: Option<u64>,
}
impl Default for RunAccounting {
    fn default() -> Self {
        // START with no model attempt has a known zero; an attempted step with no report does not.
        Self {
            steps: Some(0),
            credits: Some(0),
        }
    }
}
impl RunAccounting {
    pub(super) fn record(&mut self, step: &Accounting) {
        self.steps = self
            .steps
            .and_then(|n| n.checked_add(1))
            .filter(|n| *n <= MAX_SAFE_INTEGER);
        // SDK rounds per model callback, not after summing dollar observations across completions.
        // Once a step is unknown/overflowed, later known observations cannot restore a false total.
        self.credits = self
            .credits
            .and_then(|n| n.checked_add(step.sdk_credits().ok()?))
            .filter(|n| *n <= MAX_SAFE_INTEGER);
    }
    pub(super) fn totals(&self) -> Result<RunTotals, BusinessError> {
        let total_steps = self.steps.ok_or(BusinessError::AccountingUnknown)?;
        let credits = self.credits.ok_or(BusinessError::AccountingUnknown)?;
        // This lane has only sequential model steps. A future child/tool owner must add its actual
        // reported obligations; direct=total is not a claim about a complete agent harness.
        Ok(RunTotals {
            total_steps,
            direct_credits: credits,
            total_credits: credits,
        })
    }
}
pub(crate) struct FinishedCatalogRun {
    pub(crate) completion: Result<Value, BusinessError>,
    /// Only the last step's observations; totals are carried separately and never replace them.
    pub(crate) accounting: Accounting,
    pub(crate) totals: RunTotals,
    /// Transport acceptance only, not a settlement receipt or a session release.
    pub(crate) finish_http_status: u16,
}
async fn finish(
    mut run: CatalogRun<'_>,
    completion: Result<Value, BusinessError>,
    accounting: Accounting,
    budget: Duration,
) -> Result<FinishedCatalogRun, BusinessError> {
    let totals = run.accounting.totals()?;
    run.journal
        .finishing(completion.as_ref().err().copied())
        .await?;
    // Finalization must remain possible after an observation expires. This is the original run's
    // accounting transport, not permission for another completion or a new session purchase.
    let finish_http_status = finish_wire::finish(
        run.requests,
        &run.run_id,
        totals,
        completion.as_ref().err().copied(),
        budget,
    )
    .await?;
    run.journal.accepted(finish_http_status).await?;
    Ok(FinishedCatalogRun {
        completion,
        accounting,
        totals,
        finish_http_status,
    })
}
impl CatalogRun<'_> {
    /// Cooperative host cancellation BETWEEN steps, including zero steps after a confirmed START.
    /// Dropping a future is still not an implicit FINISH or a promise of durable run recovery.
    pub(crate) async fn finish_cancelled(
        self,
        budget: Duration,
    ) -> Result<FinishedCatalogRun, BusinessError> {
        finish(
            self,
            Err(BusinessError::Cancelled),
            Accounting::default(),
            budget,
        )
        .await
    }
}
impl<'a> DecodedCatalogRun<'a> {
    /// Return the same unique run only after full semantic success and real EOF. A failed/cancelled
    /// result retains the owner in Err so the host may explicitly finalize its reported partial cost.
    pub(crate) fn continue_run(self) -> Result<(CatalogRun<'a>, Value), Self> {
        if self.completion.is_err() {
            return Err(self);
        }
        Ok((self.run, self.completion.expect("checked decoded result")))
    }
    pub(crate) async fn finish_run(
        self,
        budget: Duration,
    ) -> Result<FinishedCatalogRun, BusinessError> {
        finish(self.run, self.completion, self.accounting, budget).await
    }
    /// Keep the old method's exact one-step contract; it must not silently truncate a longer run.
    pub(crate) async fn finish_single_step(
        self,
        budget: Duration,
    ) -> Result<FinishedCatalogRun, BusinessError> {
        if self.run.accounting.totals()?.total_steps != 1 {
            return Err(BusinessError::InvalidInput);
        }
        self.finish_run(budget).await
    }
}
#[cfg(test)]
#[path = "sequence_tests.rs"]
mod tests;
