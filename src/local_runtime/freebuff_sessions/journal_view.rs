//! Offline account-bound journal facts. No OS slot capability, device repair or upstream request.
use super::*;
use crate::protocol::freebuff::catalog::SessionEndReceipt;
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StoredClaimView {
    pub(crate) attempt_id: String,
    pub(crate) model_key: String,
    run: Option<run_store::RunView>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ClosedClaimView {
    attempt_id: String,
    model_key: String,
    receipt: SessionEndReceipt,
    run: Option<run_store::RunView>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ClaimJournalView {
    pub(crate) claim: Option<StoredClaimView>,
    history: Vec<ClosedClaimView>,
    next_cursor: Option<String>,
}
impl LocalRuntime {
    /// A consistent read snapshot, including while another owner holds the execution guard.
    /// Returned attempt IDs select later operations; they never authorize or reconstruct a claim.
    pub(crate) async fn freebuff_claim_journal(
        &self,
        provider: &str,
        credential: &str,
        scope: &StoredClaimScope,
        cursor: Option<uuid::Uuid>,
    ) -> Result<ClaimJournalView, GatewayError> {
        enabled()?;
        let binding = binding(provider, credential);
        let mut tx = self.pool.begin().await.map_err(storage_error)?;
        let claim = match load(&mut tx, &self.secret_seal, &binding, scope).await? {
            Some((claim, model_key, sealed)) => Some(StoredClaimView {
                run: run_store::read(self, &mut tx, &binding, claim.attempt_id(), &sealed).await?,
                attempt_id: claim.attempt_id().into(),
                model_key,
            }),
            None => None,
        };
        let before: Option<i64> = match cursor {
            None => None,
            Some(cursor) => Some(sqlx::query_scalar("SELECT rowid FROM local_freebuff_session_closeouts WHERE binding=? AND attempt_id=?")
                .bind(&binding).bind(cursor.to_string()).fetch_optional(&mut *tx).await.map_err(storage_error)?
                .ok_or_else(|| GatewayError::bad_request("FreeBuff history cursor does not belong to this saved account.").with_code("freebuff_session_history_cursor_invalid"))?),
        };
        let mut rows: Vec<(String, String, String)> = sqlx::query_as("SELECT CASE WHEN length(attempt_id)=36 THEN attempt_id ELSE '' END,
            CASE WHEN length(claim_sealed)<=8192 THEN claim_sealed ELSE '' END,
            CASE WHEN length(receipt_sealed)<=8192 THEN receipt_sealed ELSE '' END
            FROM local_freebuff_session_closeouts WHERE binding=? AND (? IS NULL OR rowid<?) ORDER BY rowid DESC LIMIT 21")
            .bind(&binding).bind(before).bind(before).fetch_all(&mut *tx).await.map_err(storage_error)?;
        let more = rows.len() > 20;
        rows.truncate(20);
        let mut history = Vec::with_capacity(rows.len());
        for (attempt_id, claim, receipt) in rows {
            let closed =
                self.restore_closed_claim(&binding, &attempt_id, &claim, &receipt, scope)?;
            history.push(ClosedClaimView {
                run: run_store::read(self, &mut tx, &binding, &attempt_id, &claim).await?,
                attempt_id,
                model_key: closed.model_key,
                receipt: closed.receipt,
            });
        }
        let next_cursor = if more {
            history.last().map(|row| row.attempt_id.clone())
        } else {
            None
        };
        tx.commit().await.map_err(storage_error)?;
        Ok(ClaimJournalView {
            claim,
            history,
            next_cursor,
        })
    }
}
