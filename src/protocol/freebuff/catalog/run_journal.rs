//! Encrypted-store DTOs describe facts, never execution/consent capabilities or retry instructions.
pub(crate) use super::execution::sequence::RunTotals;
use super::{completion_decode::MAX_SAFE_INTEGER, BusinessError};
use futures::future::BoxFuture;
use serde::{Deserialize, Serialize};

pub(crate) trait RunRecorder: Send + Sync {
    fn save<'a>(&'a mut self, record: &'a RunRecord) -> BoxFuture<'a, Result<(), BusinessError>>;
}
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RunPhase {
    StartPending,
    Started,
    ModelPending,
    ModelObserved,
    StepStopped,
    FinishPending,
    FinishAccepted,
}
#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum FinishStatus {
    Completed,
    Cancelled,
    Failed,
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct RunCost {
    pub(crate) cost: Option<f64>,
    pub(crate) upstream_estimate: Option<f64>,
    pub(crate) sdk_credits: Option<u64>,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct RunRecord {
    pub(crate) version: u8,
    pub(crate) client_id: String,
    pub(crate) run_id: Option<String>,
    pub(crate) phase: RunPhase,
    pub(crate) totals: Option<RunTotals>,
    #[serde(default)]
    pub(crate) prior_totals: Option<RunTotals>,
    pub(crate) last_step: Option<RunCost>,
    pub(crate) last_error: Option<String>,
    pub(crate) finish_status: Option<FinishStatus>,
    pub(crate) finish_http_status: Option<u16>,
}
impl RunRecord {
    /// Storage is a trust boundary even though only this binary seals new records.
    pub(crate) fn validate(&self) -> Result<(), BusinessError> {
        let start = self.phase == RunPhase::StartPending;
        let finish = matches!(
            self.phase,
            RunPhase::FinishPending | RunPhase::FinishAccepted
        );
        let accepted = self.phase == RunPhase::FinishAccepted;
        let zero = self
            .totals
            .is_some_and(|n| n.total_steps == 0 && n.total_credits == 0);
        let valid = self.version == 1
            && super::atom(&self.client_id, 4096)
            && (if start {
                self.run_id.is_none()
            } else {
                self.run_id
                    .as_deref()
                    .is_some_and(|id| super::atom(id, 4096))
            })
            && self.totals.is_none_or(|n| {
                n.total_steps <= MAX_SAFE_INTEGER
                    && n.total_credits <= MAX_SAFE_INTEGER
                    && n.direct_credits == n.total_credits
            })
            && self.prior_totals.is_none_or(|n| {
                n.total_steps <= MAX_SAFE_INTEGER
                    && n.total_credits <= MAX_SAFE_INTEGER
                    && n.direct_credits == n.total_credits
            })
            && (!matches!(self.phase, RunPhase::StartPending | RunPhase::Started) || zero)
            && (self.phase != RunPhase::ModelPending || self.totals.is_none())
            && (finish == self.finish_status.is_some())
            && (!finish || self.totals.is_some())
            && (if accepted {
                self.finish_http_status
                    .is_some_and(|n| (200..300).contains(&n))
            } else {
                self.finish_http_status.is_none()
            })
            && self
                .last_error
                .as_deref()
                .is_none_or(|code| super::atom(code, 128));
        if !valid {
            return Err(BusinessError::RunJournalUnavailable);
        }
        if let Some(cost) = &self.last_step {
            let observation = super::completion_decode::Accounting {
                cost: cost.cost,
                upstream_estimate: cost.upstream_estimate,
            };
            if [cost.cost, cost.upstream_estimate]
                .into_iter()
                .flatten()
                .any(|n| !n.is_finite() || n < 0.0)
                || (cost.cost.is_none() && cost.upstream_estimate.is_none())
                || observation.sdk_credits().ok() != cost.sdk_credits
            {
                return Err(BusinessError::RunJournalUnavailable);
            }
        }
        Ok(())
    }
}
