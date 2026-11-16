//! Awaited checkpoints on the original run. A dropped/failed write disables all later sends.
use super::super::{
    completion_decode::Accounting,
    run_journal::{FinishStatus, RunCost, RunPhase, RunRecord, RunRecorder},
};
use super::{sequence::RunAccounting, BusinessError};

pub(super) struct RunJournal<'a> {
    pub(super) record: RunRecord,
    sink: Option<Box<dyn RunRecorder + 'a>>,
    failed: bool,
}
#[cfg(all(test, feature = "line-freebuff-web-reverse-api"))]
#[path = "journal_tests.rs"]
mod tests;
impl<'a> RunJournal<'a> {
    pub(super) fn new(client_id: String, sink: Option<Box<dyn RunRecorder + 'a>>) -> Self {
        Self {
            record: RunRecord {
                version: 1,
                client_id,
                run_id: None,
                phase: RunPhase::StartPending,
                totals: RunAccounting::default().totals().ok(),
                prior_totals: None,
                last_step: None,
                last_error: None,
                finish_status: None,
                finish_http_status: None,
            },
            sink,
            failed: false,
        }
    }
    pub(super) async fn save(&mut self) -> Result<(), BusinessError> {
        if self.failed {
            return Err(BusinessError::RunJournalUnavailable);
        }
        self.record.validate()?;
        if let Some(sink) = &mut self.sink {
            // If a select cancels this await, the store commit is unknown. Never proceed to FINISH.
            self.failed = true;
            sink.save(&self.record).await?;
            self.failed = false;
        }
        Ok(())
    }
    pub(super) async fn model_pending(
        &mut self,
        ledger: &RunAccounting,
    ) -> Result<(), BusinessError> {
        self.record.phase = RunPhase::ModelPending;
        // Unknown current spend must not erase the already recorded prefix from earlier steps.
        self.record.prior_totals = ledger.totals().ok().or(self.record.prior_totals);
        self.record.totals = None;
        self.record.last_step = None;
        self.record.last_error = None;
        self.save().await
    }
    pub(super) async fn observe(
        &mut self,
        ledger: &RunAccounting,
        cost: &Accounting,
    ) -> Result<(), BusinessError> {
        if cost.cost.is_none() && cost.upstream_estimate.is_none() {
            return Ok(());
        }
        let observation = RunCost {
            cost: cost.cost,
            upstream_estimate: cost.upstream_estimate,
            sdk_credits: cost.sdk_credits().ok(),
        };
        if self.record.last_step.as_ref() == Some(&observation) {
            return Ok(());
        }
        self.record.last_step = Some(observation);
        let mut projected = ledger.clone();
        projected.record(cost);
        self.record.totals = projected.totals().ok();
        self.record.phase = RunPhase::ModelObserved;
        self.save().await
    }
    pub(super) async fn stopped(
        &mut self,
        ledger: &RunAccounting,
        error: Option<BusinessError>,
    ) -> Result<(), BusinessError> {
        self.record.phase = RunPhase::StepStopped;
        self.record.totals = ledger.totals().ok();
        self.record.last_error = error.map(|e| e.to_string());
        self.save().await
    }
    pub(super) async fn finishing(
        &mut self,
        error: Option<BusinessError>,
    ) -> Result<(), BusinessError> {
        self.record.phase = RunPhase::FinishPending;
        self.record.finish_status = Some(match error {
            None => FinishStatus::Completed,
            Some(BusinessError::Cancelled) => FinishStatus::Cancelled,
            Some(_) => FinishStatus::Failed,
        });
        self.record.last_error = error.map(|e| e.to_string());
        self.save().await
    }
    pub(super) async fn accepted(&mut self, status: u16) -> Result<(), BusinessError> {
        self.record.phase = RunPhase::FinishAccepted;
        self.record.finish_http_status = Some(status);
        self.save().await
    }
}
