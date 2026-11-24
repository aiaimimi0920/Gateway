//! Server-only Gateway probe audits; no public caller identity, payloads, or billing writes.
use super::InternalProbeProduct;
use crate::{db, error::GatewayError};
use sqlx::PgPool;

#[derive(Clone)]
pub(super) struct InternalStore {
    pool: PgPool,
    product: InternalProbeProduct,
    namespace: String,
    credential_id: String,
}

pub(super) fn unavailable() -> GatewayError {
    GatewayError::server_error("Internal model probe audit unavailable.")
        .with_code("internal_model_probe_audit_unavailable")
}

impl InternalStore {
    pub(super) fn new(
        pool: PgPool,
        product: InternalProbeProduct,
        namespace: &str,
        credential_id: &str,
    ) -> Self {
        Self {
            pool,
            product,
            namespace: namespace.into(),
            credential_id: credential_id.into(),
        }
    }

    pub(super) async fn create(
        &self,
        input: db::CreateRequestAuditInput,
    ) -> Result<String, GatewayError> {
        let provider = input
            .provider_account_id
            .as_deref()
            .ok_or_else(unavailable)?;
        let model = input.resolved_model.as_deref().ok_or_else(unavailable)?;
        let id = uuid::Uuid::new_v4().to_string();
        // Project/caller identities, arbitrary trace/error data and response IDs are not persisted.
        let sap = matches!(self.product, InternalProbeProduct::SapAiCore);
        // NULL means unknown, including a process lost after send but before finalization.
        // Referencing the additive column also makes missing SAP migration fail before send.
        sqlx::query(if sap {
            "INSERT INTO gateway_internal_model_probe_audits
            (id, namespace, product, provider_id, credential_id, model, sap_evidence)
            VALUES ($1, $2, $3, $4, $5, $6, NULL)"
        } else {
            "INSERT INTO gateway_internal_model_probe_audits
            (id, namespace, product, provider_id, credential_id, model)
            VALUES ($1, $2, $3, $4, $5, $6)"
        })
        .bind(&id)
        .bind(&self.namespace)
        .bind(self.product.label())
        .bind(provider)
        .bind(&self.credential_id)
        .bind(model)
        .execute(&self.pool)
        .await
        .map_err(|_| unavailable())?;
        Ok(id)
    }

    pub(super) async fn finalize(
        &self,
        id: &str,
        input: db::FinalizeRequestAuditInput,
    ) -> Result<(), GatewayError> {
        let status = match input.status.as_str() {
            status @ ("completed" | "failed" | "cancelled") => status,
            _ => return Err(unavailable()),
        };
        let sap = matches!(self.product, InternalProbeProduct::SapAiCore);
        let mut query = sqlx::query(if sap {
            "UPDATE gateway_internal_model_probe_audits SET
            status = $4, upstream_status = $5, duration_ms = $6, prompt_tokens = $7,
            completion_tokens = $8, total_tokens = $9, completed_at = clock_timestamp(),
            sap_evidence = $11
            WHERE id = $1 AND namespace = $2 AND credential_id = $3
              AND product = $10 AND status = 'running'"
        } else {
            "UPDATE gateway_internal_model_probe_audits SET
            status = $4, upstream_status = $5, duration_ms = $6, prompt_tokens = $7,
            completion_tokens = $8, total_tokens = $9, completed_at = clock_timestamp()
            WHERE id = $1 AND namespace = $2 AND credential_id = $3
              AND product = $10 AND status = 'running'"
        })
        .bind(id)
        .bind(&self.namespace)
        .bind(&self.credential_id)
        .bind(status)
        .bind(input.upstream_status.map(i32::from))
        .bind(to_bigint(input.duration_ms))
        .bind(input.prompt_tokens.map(to_bigint))
        .bind(input.completion_tokens.map(to_bigint))
        .bind(input.total_tokens.map(to_bigint))
        .bind(self.product.label());
        if sap {
            // Produced exclusively by the SAP probe owner, including unclaimed-create cancellation.
            let evidence = input
                .route_trace
                .as_ref()
                .and_then(|trace| trace.get("sapProbe"))
                .ok_or_else(unavailable)?;
            query = query.bind(evidence);
        }
        let result = query.execute(&self.pool).await.map_err(|_| unavailable())?;
        if result.rows_affected() != 1 {
            return Err(unavailable());
        }
        Ok(())
    }
}

fn to_bigint(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}
