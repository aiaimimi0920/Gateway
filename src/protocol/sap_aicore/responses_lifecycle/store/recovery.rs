//! Local ledger recovery only: no provider authorization, OAuth, GET, or replay.
//! A selection is a hint; each mutation rechecks durable evidence under the receipt lock.
use super::{balance::sql, storage, Ledger, Receipt, Store, COLUMNS};
use crate::error::GatewayError;

mod worker;
pub(crate) use worker::spawn;
#[cfg(test)]
pub(super) use worker::tick;
const PAGE: usize = 32;

impl Store<'_> {
    /// Only the live submission owner before execute may call this method.
    pub(super) async fn record_unsent(&self, receipt: &Receipt) -> Result<(), GatewayError> {
        let mut tx = self.ledger().await?;
        let row = tx.lock_receipt(receipt).await?;
        if row.phase != "submitting"
            || row.upstream_id.is_some()
            || row.revision != receipt.revision
        {
            return Err(proof_conflict());
        }
        let held: Option<i64> = sql!(&mut tx, conn => sqlx::query_scalar::<_, i64>(
            "SELECT open FROM gateway_sap_response_balance_holds WHERE response_id=$1")
            .bind(&row.id).fetch_optional(&mut **conn).await)?;
        if held == Some(0) {
            return Ok(());
        }
        if held != Some(1) {
            return Err(proof_conflict());
        }
        sql!(&mut tx, conn => sqlx::query(
            "INSERT INTO gateway_sap_response_unsent_proofs (response_id,revision) VALUES ($1,$2)
             ON CONFLICT(response_id) DO NOTHING")
            .bind(&row.id).bind(row.revision).execute(&mut **conn).await.map(|_| ()))?;
        if !tx.proven_unsent(&row).await? {
            return Err(proof_conflict());
        }
        tx.commit().await
    }

    async fn recoverable(&self, after: &str) -> Result<Vec<Receipt>, GatewayError> {
        let sql = format!(
            "SELECT {COLUMNS}
            FROM gateway_sap_response_balance_holds h
            JOIN gateway_sap_response_receipts r ON r.id=h.response_id
            WHERE h.open=1 AND h.response_id > $1 AND (
                (r.phase IN ('completed','failed') AND r.settled=0
                    AND r.input_tokens IS NOT NULL AND r.output_tokens IS NOT NULL)
                OR (r.phase='completed' AND r.settled=0
                    AND r.input_tokens IS NULL AND r.output_tokens IS NULL
                    AND h.mode='message_prepaid' AND h.reserved=1)
                OR (r.phase='submitting' AND r.upstream_id IS NULL AND EXISTS(
                    SELECT 1 FROM gateway_sap_response_unsent_proofs p
                    WHERE p.response_id=r.id AND p.revision=r.revision)))
            ORDER BY h.response_id LIMIT {PAGE}"
        );
        run!(self, pool => sqlx::query_as::<_, Receipt>(&sql).bind(after).fetch_all(*pool).await)
    }

    async fn recover(&self, row: &Receipt) -> Result<bool, GatewayError> {
        if row.phase == "submitting" {
            self.refund_proven_unsent(row).await
        } else if row.terminal() {
            // Accounting does not grant inference authority. Original account/mode checks
            // still apply; receipt settlement never releases an unconfirmed mutation.
            self.settle_recovered_balance(row)
                .await
                .map(|result| result.is_some())
        } else {
            Ok(false)
        }
    }
}

impl Ledger<'_> {
    pub(super) async fn proven_unsent(&mut self, row: &Receipt) -> Result<bool, GatewayError> {
        sql!(self, conn => sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM gateway_sap_response_unsent_proofs
             WHERE response_id=$1 AND revision=$2)")
            .bind(&row.id).bind(row.revision).fetch_one(&mut **conn).await)
    }
}

fn proof_conflict() -> GatewayError {
    GatewayError::conflict("SAP unsent evidence does not match the receipt.")
        .with_code("sap_aicore_response_unsent_conflict")
}

#[cfg(test)]
pub(super) mod message_tests;
#[cfg(test)]
pub(super) mod tests;
