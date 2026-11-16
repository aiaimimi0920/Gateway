//! Local prepare-before-send owner for FreeBuff paid-session claims.
//! No management purchase route or automatic consent. Every existing row remains unresolved until
//! the original claim is reconciled; no timeout/expiry/foreign response authorizes another POST.
use super::{storage_error, LocalRuntime};
use crate::{
    error::GatewayError,
    protocol::freebuff::catalog::{
        AdmissionRequest, BusinessError, BusinessRequests, RecoveryRequests, SessionChargeConsent,
        SessionClaim, StoredClaimScope,
    },
};
use std::fs::File;
mod closeout;
mod journal_view;
mod lifecycle;
mod lock;
mod purchase;
mod run_store;
pub(crate) use lifecycle::ActiveAdmission;

pub(super) const SCHEMA: &str = "CREATE TABLE IF NOT EXISTS local_freebuff_session_claims (
    binding TEXT PRIMARY KEY, attempt_id TEXT NOT NULL, sealed TEXT NOT NULL);
    CREATE TABLE IF NOT EXISTS local_freebuff_session_closeouts (
    binding TEXT NOT NULL, attempt_id TEXT NOT NULL, claim_sealed TEXT NOT NULL,
    receipt_sealed TEXT NOT NULL, PRIMARY KEY(binding,attempt_id));
    CREATE TABLE IF NOT EXISTS local_freebuff_runs (
    binding TEXT NOT NULL, attempt_id TEXT NOT NULL, claim_sealed TEXT NOT NULL,
    sealed TEXT NOT NULL, PRIMARY KEY(binding,attempt_id));";
pub(crate) enum AdmissionPreparation<'a> {
    Send(PendingAdmission<'a>),
    Recover(SessionRecovery),
}
pub(crate) struct PendingAdmission<'a> {
    request: AdmissionRequest<'a>,
    owner: SessionRecovery,
}
pub(crate) struct SessionRecovery {
    _guard: File,
    claim: SessionClaim,
    model_key: String,
    runtime: LocalRuntime,
    binding: String,
    sealed: String,
}
fn unavailable() -> GatewayError {
    GatewayError::service_unavailable("FreeBuff session claim storage is unavailable or belongs to an earlier account authority. Restore the original state and seal before reconciling; no new session was purchased.")
        .with_code("freebuff_session_claim_unavailable")
}
fn business_error(error: BusinessError) -> GatewayError {
    GatewayError::service_unavailable("FreeBuff admission is unconfirmed. Reconcile the original claim; do not purchase another session.").with_code(error.to_string())
}
// Deliberately stable across token/origin/client changes. A changed authority must block, not look absent.
fn binding(provider: &str, credential: &str) -> String {
    let mut hash = ring::digest::Context::new(&ring::digest::SHA256);
    hash.update(b"gateway-freebuff-session-slot:v1");
    for bytes in [provider.as_bytes(), credential.as_bytes()] {
        hash.update(&(bytes.len() as u64).to_be_bytes());
        hash.update(bytes);
    }
    hex::encode(hash.finish())
}
fn aad(binding: &str) -> String {
    format!("gateway-freebuff-session-claim:v1:{binding}")
}
fn enabled() -> Result<(), GatewayError> {
    if !cfg!(feature = "line-freebuff-web-reverse-api") {
        return Err(
            GatewayError::service_unavailable("FreeBuff provider line is compiled out.")
                .with_code("gateway_provider_line_compiled_out"),
        );
    }
    Ok(())
}
async fn load(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    seal: &super::secret_seal::LocalSecretSeal,
    binding: &str,
    scope: &StoredClaimScope,
) -> Result<Option<(SessionClaim, String, String)>, GatewayError> {
    let row: Option<(String, String)> = sqlx::query_as(
        "SELECT CASE WHEN length(attempt_id)=36 THEN attempt_id ELSE '' END,
         CASE WHEN length(sealed)<=8192 THEN sealed ELSE '' END
         FROM local_freebuff_session_claims WHERE binding=?",
    )
    .bind(binding)
    .fetch_optional(&mut **tx)
    .await
    .map_err(storage_error)?;
    let Some((attempt, sealed)) = row else {
        return Ok(None);
    };
    let bytes = seal
        .open_bytes(aad(binding).as_bytes(), &sealed, 8192, 4096)
        .map_err(|_| unavailable())?;
    let (claim, model) = scope.restore_claim(&bytes).map_err(|_| unavailable())?;
    if claim.attempt_id() != attempt {
        return Err(unavailable());
    }
    Ok(Some((claim, model, sealed)))
}
impl LocalRuntime {
    /// Test-only access to the lower-level journal contract; production must use the quote owner.
    #[cfg(test)]
    pub(crate) async fn prepare_freebuff_admission<'a>(
        &self,
        provider: &str,
        credential: &str,
        requests: &'a BusinessRequests<'a>,
        consent: SessionChargeConsent,
    ) -> Result<AdmissionPreparation<'a>, GatewayError> {
        self.prepare_admission_checked(provider, credential, requests, consent, || Ok(()))
            .await
    }
    /// Only the composed quote owner can prepare a production purchase.
    /// Commit happens before the only object capable of sending admission escapes this function.
    async fn prepare_admission_checked<'a>(
        &self,
        provider: &str,
        credential: &str,
        requests: &'a BusinessRequests<'a>,
        consent: SessionChargeConsent,
        check: impl Fn() -> Result<(), GatewayError>,
    ) -> Result<AdmissionPreparation<'a>, GatewayError> {
        enabled()?;
        let binding = binding(provider, credential);
        let guard = lock::acquire(self, &binding).await?;
        let mut tx = self
            .pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(storage_error)?;
        if let Some((claim, model_key, sealed)) = load(
            &mut tx,
            &self.secret_seal,
            &binding,
            &requests.recovery_requests().stored_claim_scope(),
        )
        .await?
        {
            tx.commit().await.map_err(storage_error)?;
            return Ok(AdmissionPreparation::Recover(SessionRecovery {
                _guard: guard,
                claim,
                model_key,
                runtime: self.clone(),
                binding,
                sealed,
            }));
        }
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM local_freebuff_session_claims")
            .fetch_one(&mut *tx)
            .await
            .map_err(storage_error)?;
        if count >= 4096 {
            return Err(GatewayError::conflict(
                "FreeBuff session claim capacity reached; unresolved purchases were retained.",
            )
            .with_code("freebuff_session_claim_capacity"));
        }
        check()?;
        let request = requests
            .prepare_cli_admission(consent)
            .map_err(business_error)?;
        let bytes = requests
            .claim_receipt(request.claim())
            .map_err(business_error)?;
        let (_, model_key) = requests
            .recovery_requests()
            .restore_claim(&bytes)
            .map_err(business_error)?;
        let sealed = self
            .secret_seal
            .seal(aad(&binding).as_bytes(), &bytes, 4096)
            .map_err(|_| unavailable())?;
        sqlx::query(
            "INSERT INTO local_freebuff_session_claims(binding,attempt_id,sealed) VALUES (?,?,?)",
        )
        .bind(&binding)
        .bind(request.claim().attempt_id())
        .bind(&sealed)
        .execute(&mut *tx)
        .await
        .map_err(storage_error)?;
        #[cfg(all(test, feature = "line-freebuff-web-reverse-api"))]
        checkpoint(false).await;
        check()?;
        tx.commit().await.map_err(storage_error)?;
        #[cfg(all(test, feature = "line-freebuff-web-reverse-api"))]
        checkpoint(true).await;
        let owner = SessionRecovery {
            _guard: guard,
            claim: request.claim().clone(),
            model_key,
            runtime: self.clone(),
            binding,
            sealed,
        };
        Ok(AdmissionPreparation::Send(PendingAdmission {
            request,
            owner,
        }))
    }
    /// No registration, model selection, POST, implicit end or database cleanup. Existing records
    /// survive missing authority/seal, clock changes and a catalog that no longer offers the model.
    pub(crate) async fn recover_freebuff_admission(
        &self,
        provider: &str,
        credential: &str,
        requests: &RecoveryRequests<'_>,
    ) -> Result<Option<SessionRecovery>, GatewayError> {
        enabled()?;
        let binding = binding(provider, credential);
        let guard = lock::acquire(self, &binding).await?;
        let mut tx = self.pool.begin().await.map_err(storage_error)?;
        let row = load(
            &mut tx,
            &self.secret_seal,
            &binding,
            &requests.stored_claim_scope(),
        )
        .await?;
        tx.commit().await.map_err(storage_error)?;
        Ok(row.map(|(claim, model_key, sealed)| SessionRecovery {
            _guard: guard,
            claim,
            model_key,
            runtime: self.clone(),
            binding,
            sealed,
        }))
    }
}
#[cfg(all(test, feature = "line-freebuff-web-reverse-api"))]
tokio::task_local! {
    pub(crate) static TEST_PREPARE_CHECKPOINT: (bool, std::sync::Arc<tokio::sync::Notify>, std::sync::Arc<tokio::sync::Notify>);
}
#[cfg(all(test, feature = "line-freebuff-web-reverse-api"))]
async fn checkpoint(after_commit: bool) {
    if let Ok((point, reached, resume)) = TEST_PREPARE_CHECKPOINT.try_with(Clone::clone) {
        if point == after_commit {
            reached.notify_one();
            resume.notified().await;
        }
    }
}
#[cfg(all(test, feature = "line-freebuff-web-reverse-api"))]
mod tests;
