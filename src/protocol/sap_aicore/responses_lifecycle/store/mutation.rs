//! One durable dispatch authorization per response and verb, shared by all instances.
use super::{
    balance::{sql, Ledger},
    storage, Receipt, Store, UpstreamId,
};
use crate::error::GatewayError;
use serde_json::Value;

#[derive(Clone, Copy)]
pub(super) enum MutationKind {
    Cancel,
    Delete,
}

impl MutationKind {
    fn key(self) -> &'static str {
        match self {
            Self::Cancel => "cancel",
            Self::Delete => "delete",
        }
    }
}

pub(super) struct MutationClaim {
    receipt: Receipt,
    upstream: UpstreamId,
    kind: MutationKind,
}

impl Store<'_> {
    /// The caller must freshly authorize the exact stored binding before claiming.
    /// A duplicate never grants dispatch, even when the original result is unknown.
    pub(super) async fn claim_mutation(
        &self,
        receipt: &Receipt,
        kind: MutationKind,
    ) -> Result<Option<MutationClaim>, GatewayError> {
        let mut tx = self.ledger().await?;
        let row = tx.lock_receipt(receipt).await?;
        if row.revision != receipt.revision
            || match kind {
                MutationKind::Cancel => row.phase != "pending",
                MutationKind::Delete => !row.terminal() || row.settled != 1,
            }
        {
            return Ok(None);
        }
        let upstream = row
            .upstream_id
            .as_deref()
            .and_then(UpstreamId::parse)
            .ok_or_else(super::super::invalid)?;
        let inserted = sql!(&mut tx, conn => sqlx::query(
            "INSERT INTO gateway_sap_response_mutations (response_id, operation)
             SELECT $1, $2 WHERE NOT EXISTS (
                 SELECT 1 FROM gateway_sap_response_mutations
                 WHERE response_id = $1 AND confirmed = 0
             ) ON CONFLICT (response_id, operation) DO NOTHING"
        )
        .bind(&row.id)
        .bind(kind.key())
        .execute(&mut **conn)
        .await
        .map(|result| result.rows_affected()))?;
        tx.commit().await?;
        Ok((inserted == 1).then_some(MutationClaim {
            receipt: row,
            upstream,
            kind,
        }))
    }

    pub(super) async fn confirm_mutation(
        &self,
        claim: MutationClaim,
        body: &Value,
    ) -> Result<(), GatewayError> {
        let receipt = claim.receipt;
        match claim.kind {
            MutationKind::Cancel => {
                let (upstream, state) = super::super::observe(body, Some(&claim.upstream))?;
                // A pending reply does not prove the cancel is finished. Keep
                // its durable barrier, even if an independent poll is terminal.
                if matches!(state, super::super::State::Pending) {
                    return Ok(());
                }
                // A racing retrieve may already have recorded the terminal state.
                // Persist any terminal observation before allowing a later delete.
                if self
                    .observe(&receipt, &upstream, state.clone())
                    .await?
                    .is_none()
                {
                    if let Some(current) = self.read(&receipt.id, &receipt.owner_tag).await? {
                        // The failed CAS can be a poll that reached the same terminal
                        // phase without usage. Give this verified reply one chance to fill it.
                        if current.binding_tag == receipt.binding_tag {
                            self.observe(&current, &upstream, state).await?;
                        }
                    }
                }
            }
            MutationKind::Delete => super::super::validate_deleted(body, &claim.upstream)?,
        }

        let mut tx = self.ledger().await?;
        let row = tx.lock_receipt(&receipt).await?;
        if matches!(claim.kind, MutationKind::Delete) {
            // Retain the tombstone and the durable claim as separate evidence.
            let deleted = sql!(&mut tx, conn => sqlx::query(
                "UPDATE gateway_sap_response_receipts SET phase = 'deleted', revision = revision + 1
                 WHERE id = $1 AND revision = $2 AND phase IN ('completed', 'failed') AND settled = 1"
            )
            .bind(&row.id)
            .bind(receipt.revision)
            .execute(&mut **conn)
            .await
            .map(|result| result.rows_affected()))?;
            if deleted != 1 {
                return Err(conflict());
            }
        }
        let confirmed = sql!(&mut tx, conn => sqlx::query(
            "UPDATE gateway_sap_response_mutations SET confirmed = 1
             WHERE response_id = $1 AND operation = $2 AND confirmed = 0"
        )
        .bind(&row.id)
        .bind(claim.kind.key())
        .execute(&mut **conn)
        .await
        .map(|result| result.rows_affected()))?;
        if confirmed != 1 {
            return Err(conflict());
        }
        tx.commit().await
    }

    /// Only the still-owned, never-executed request can release this claim.
    pub(super) async fn release_unsent(&self, claim: MutationClaim) -> Result<(), GatewayError> {
        let mut tx = self.ledger().await?;
        let row = tx.lock_receipt(&claim.receipt).await?;
        let released = sql!(&mut tx, conn => sqlx::query(
            "DELETE FROM gateway_sap_response_mutations
             WHERE response_id = $1 AND operation = $2 AND confirmed = 0"
        )
        .bind(&row.id)
        .bind(claim.kind.key())
        .execute(&mut **conn)
        .await
        .map(|result| result.rows_affected()))?;
        if released != 1 {
            return Err(conflict());
        }
        tx.commit().await
    }
}

fn conflict() -> GatewayError {
    GatewayError::conflict("Stored SAP Responses mutation claim does not match")
        .with_code("sap_aicore_response_mutation_conflict")
}
