//! One encrypted run per original claim. Atomic preimage CAS precedes every irreversible wire phase.
use super::*;
use crate::protocol::freebuff::catalog::run_journal::{RunRecord, RunRecorder};
use futures::future::BoxFuture;
use serde::Serialize;

pub(super) struct RunStore {
    runtime: LocalRuntime,
    binding: String,
    attempt: String,
    claim_sealed: String,
    sealed: Option<String>,
}
fn run_aad(binding: &str, attempt: &str) -> String {
    format!("gateway-freebuff-run:v1:{binding}:{attempt}")
}
impl SessionRecovery {
    pub(super) fn run_store(&self) -> RunStore {
        RunStore {
            runtime: self.runtime.clone(),
            binding: self.binding.clone(),
            attempt: self.claim.attempt_id().into(),
            claim_sealed: self.sealed.clone(),
            sealed: None,
        }
    }
}
impl RunStore {
    async fn save_record(&mut self, record: &RunRecord) -> Result<(), GatewayError> {
        enabled()?;
        record.validate().map_err(|_| unavailable())?;
        let bytes = serde_json::to_vec(record).map_err(|_| unavailable())?;
        let sealed = self
            .runtime
            .secret_seal
            .seal(
                run_aad(&self.binding, &self.attempt).as_bytes(),
                &bytes,
                16384,
            )
            .map_err(|_| unavailable())?;
        let changed = if let Some(previous) = &self.sealed {
            sqlx::query("UPDATE local_freebuff_runs SET sealed=? WHERE binding=? AND attempt_id=? AND claim_sealed=? AND sealed=?
                AND EXISTS(SELECT 1 FROM local_freebuff_session_claims WHERE binding=? AND attempt_id=? AND sealed=?)")
                .bind(&sealed).bind(&self.binding).bind(&self.attempt).bind(&self.claim_sealed).bind(previous)
                .bind(&self.binding).bind(&self.attempt).bind(&self.claim_sealed)
                .execute(&self.runtime.pool).await.map_err(storage_error)?.rows_affected()
        } else {
            // No UPSERT: a recovered claim with any previous START intent may never create another run.
            sqlx::query("INSERT INTO local_freebuff_runs(binding,attempt_id,claim_sealed,sealed)
                SELECT ?,?,?,? WHERE EXISTS(SELECT 1 FROM local_freebuff_session_claims WHERE binding=? AND attempt_id=? AND sealed=?)")
                .bind(&self.binding).bind(&self.attempt).bind(&self.claim_sealed).bind(&sealed)
                .bind(&self.binding).bind(&self.attempt).bind(&self.claim_sealed)
                .execute(&self.runtime.pool).await.map_err(storage_error)?.rows_affected()
        };
        if changed != 1 {
            return Err(unavailable());
        }
        self.sealed = Some(sealed);
        Ok(())
    }
}
impl RunRecorder for RunStore {
    fn save<'a>(&'a mut self, record: &'a RunRecord) -> BoxFuture<'a, Result<(), BusinessError>> {
        Box::pin(async move {
            self.save_record(record)
                .await
                .map_err(|_| BusinessError::RunJournalUnavailable)
        })
    }
}

/// No remote ID, client identity, body, authentication material or execution capability is returned.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RunView {
    phase: crate::protocol::freebuff::catalog::run_journal::RunPhase,
    run_id_recorded: bool,
    totals: Option<crate::protocol::freebuff::catalog::run_journal::RunTotals>,
    prior_totals: Option<crate::protocol::freebuff::catalog::run_journal::RunTotals>,
    last_step: Option<crate::protocol::freebuff::catalog::run_journal::RunCost>,
    last_error: Option<String>,
    finish_status: Option<crate::protocol::freebuff::catalog::run_journal::FinishStatus>,
    finish_http_status: Option<u16>,
}
pub(super) async fn read(
    runtime: &LocalRuntime,
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    binding: &str,
    attempt: &str,
    claim_sealed: &str,
) -> Result<Option<RunView>, GatewayError> {
    let row: Option<(String, String)> = sqlx::query_as("SELECT CASE WHEN length(claim_sealed)<=8192 THEN claim_sealed ELSE '' END,
        CASE WHEN length(sealed)<=32768 THEN sealed ELSE '' END FROM local_freebuff_runs WHERE binding=? AND attempt_id=?")
        .bind(binding).bind(attempt).fetch_optional(&mut **tx).await.map_err(storage_error)?;
    let Some((stored_claim, sealed)) = row else {
        return Ok(None);
    };
    if stored_claim != claim_sealed {
        return Err(unavailable());
    }
    let bytes = runtime
        .secret_seal
        .open_bytes(run_aad(binding, attempt).as_bytes(), &sealed, 32768, 16384)
        .map_err(|_| unavailable())?;
    let record: RunRecord = serde_json::from_slice(&bytes).map_err(|_| unavailable())?;
    record.validate().map_err(|_| unavailable())?;
    Ok(Some(RunView {
        phase: record.phase,
        run_id_recorded: record.run_id.is_some(),
        totals: record.totals,
        prior_totals: record.prior_totals,
        last_step: record.last_step,
        last_error: record.last_error,
        finish_status: record.finish_status,
        finish_http_status: record.finish_http_status,
    }))
}
#[cfg(all(test, feature = "line-freebuff-web-reverse-api"))]
mod tests;
