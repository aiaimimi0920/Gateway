//! Exact-claim end receipt and slot release are one durable transaction. No automatic release/rebuy.
use super::*;
use crate::protocol::freebuff::catalog::SessionEndReceipt;
use std::time::Duration;

pub(crate) struct ClosedAdmission {
    pub(crate) model_key: String,
    pub(crate) receipt: SessionEndReceipt,
}
fn receipt_aad(binding: &str, attempt: &str) -> String {
    format!("gateway-freebuff-session-closeout:v1:{binding}:{attempt}")
}
fn changed() -> GatewayError {
    GatewayError::conflict("FreeBuff original claim changed; no local obligation was cleared.")
        .with_code("freebuff_session_claim_changed")
}
fn end_error(error: BusinessError) -> GatewayError {
    GatewayError::service_unavailable("FreeBuff original-claim end or local closeout is unconfirmed. Inspect the original claim and closeout history before any new purchase.")
        .with_code(error.to_string())
}
impl SessionRecovery {
    /// Consume the original kernel guard. `fence` must be the normal pinned Grant/revision owner,
    /// not a body boolean. A missing/pending/grace refund retains the claim, even after HTTP 200.
    pub(crate) async fn end(
        self,
        requests: &RecoveryRequests<'_>,
        fence: impl Fn() -> Result<(), GatewayError>,
        budget: Duration,
    ) -> Result<SessionEndReceipt, GatewayError> {
        enabled()?;
        if budget.is_zero() {
            return Err(end_error(BusinessError::InvalidInput));
        }
        tokio::time::timeout(budget.min(Duration::from_secs(20)), async {
            fence()?;
            // Check the local preimage before the irreversible request, without holding a writer tx.
            let current: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM local_freebuff_session_claims WHERE binding=? AND attempt_id=? AND sealed=?)")
                .bind(&self.binding).bind(self.claim.attempt_id()).bind(&self.sealed)
                .fetch_one(&self.runtime.pool).await.map_err(storage_error)?;
            if !current {return Err(changed());}
            let reply = requests.end(&self.claim, budget).await.map_err(end_error)?;
            let receipt = SessionEndReceipt::parse(reply, &self.claim).map_err(end_error)?;
            fence()?;
            if receipt.is_final() { self.archive(&receipt, &fence).await?; }
            Ok(receipt)
        }).await.map_err(|_| end_error(BusinessError::Timeout))?
    }
    async fn archive(
        &self,
        receipt: &SessionEndReceipt,
        fence: &impl Fn() -> Result<(), GatewayError>,
    ) -> Result<(), GatewayError> {
        let attempt = self.claim.attempt_id();
        let bytes = serde_json::to_vec(receipt).map_err(|_| unavailable())?;
        let sealed = self
            .runtime
            .secret_seal
            .seal(receipt_aad(&self.binding, attempt).as_bytes(), &bytes, 4096)
            .map_err(|_| unavailable())?;
        let mut tx = self
            .runtime
            .pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(storage_error)?;
        sqlx::query("INSERT INTO local_freebuff_session_closeouts(binding,attempt_id,claim_sealed,receipt_sealed) VALUES (?,?,?,?)")
            .bind(&self.binding).bind(attempt).bind(&self.sealed).bind(sealed)
            .execute(&mut *tx).await.map_err(storage_error)?;
        let deleted = sqlx::query("DELETE FROM local_freebuff_session_claims WHERE binding=? AND attempt_id=? AND sealed=?")
            .bind(&self.binding).bind(attempt).bind(&self.sealed)
            .execute(&mut *tx).await.map_err(storage_error)?.rows_affected();
        if deleted != 1 {
            return Err(changed());
        }
        fence()?;
        tx.commit().await.map_err(storage_error)
    }
}
impl LocalRuntime {
    /// Bound, authenticated history facts only. Never restore an active capability or authorize POST.
    pub(crate) async fn freebuff_closeout(
        &self,
        provider: &str,
        credential: &str,
        requests: &RecoveryRequests<'_>,
        attempt: uuid::Uuid,
    ) -> Result<Option<ClosedAdmission>, GatewayError> {
        self.freebuff_closeout_for_scope(
            provider,
            credential,
            &requests.stored_claim_scope(),
            attempt,
        )
        .await
    }
    pub(crate) async fn freebuff_closeout_for_scope(
        &self,
        provider: &str,
        credential: &str,
        scope: &StoredClaimScope,
        attempt: uuid::Uuid,
    ) -> Result<Option<ClosedAdmission>, GatewayError> {
        enabled()?;
        let binding = binding(provider, credential);
        let attempt = attempt.to_string();
        let row: Option<(String, String)> = sqlx::query_as("SELECT CASE WHEN length(claim_sealed)<=8192 THEN claim_sealed ELSE '' END, CASE WHEN length(receipt_sealed)<=8192 THEN receipt_sealed ELSE '' END FROM local_freebuff_session_closeouts WHERE binding=? AND attempt_id=?")
            .bind(&binding).bind(&attempt).fetch_optional(&self.pool).await.map_err(storage_error)?;
        let Some((claim_sealed, receipt_sealed)) = row else {
            return Ok(None);
        };
        self.restore_closed_claim(&binding, &attempt, &claim_sealed, &receipt_sealed, scope)
            .map(Some)
    }
    pub(super) fn restore_closed_claim(
        &self,
        binding: &str,
        attempt: &str,
        claim_sealed: &str,
        receipt_sealed: &str,
        scope: &StoredClaimScope,
    ) -> Result<ClosedAdmission, GatewayError> {
        let bytes = self
            .secret_seal
            .open_bytes(aad(&binding).as_bytes(), &claim_sealed, 8192, 4096)
            .map_err(|_| unavailable())?;
        let (claim, model_key) = scope.restore_claim(&bytes).map_err(|_| unavailable())?;
        if claim.attempt_id() != attempt {
            return Err(unavailable());
        }
        let bytes = self
            .secret_seal
            .open_bytes(
                receipt_aad(&binding, &attempt).as_bytes(),
                &receipt_sealed,
                8192,
                4096,
            )
            .map_err(|_| unavailable())?;
        let receipt = SessionEndReceipt::restore(&bytes).map_err(|_| unavailable())?;
        Ok(ClosedAdmission { model_key, receipt })
    }
}
