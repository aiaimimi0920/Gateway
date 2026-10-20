//! Short-lived expected-row CAS. Tokens stay internal and never become durable revisions.
use super::{
    provider_credential_model_state_id, RecordCredentialModelFailureInput,
    RecordCredentialModelSuccessInput,
};
use crate::{db::map_db_error, error::GatewayError};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelHealthRecovery {
    Applied,
    StaleHealth,
    StaleConfiguration,
    ConfigurationChangedAfterWrite,
    StorageFailed,
    Unavailable,
    Protected,
    FailureRecorded,
    FailureNotPenalized,
}

/// Not serializable: neither a client-supplied authorization ticket nor a stored token.
pub struct ModelHealthSnapshot {
    pub(crate) identity: RecordCredentialModelSuccessInput,
    pub(crate) version: Option<String>,
    pub(crate) protected: bool,
}

impl RecordCredentialModelSuccessInput {
    pub(crate) fn state_id(&self) -> String {
        provider_credential_model_state_id(
            &self.provider_account_id,
            self.provider_credential_id.as_deref(),
            self.provider_credential_ref.as_deref(),
            self.protocol_profile.as_deref(),
            &self.model,
        )
    }
}

pub async fn snapshot(
    pool: &PgPool,
    identity: RecordCredentialModelSuccessInput,
) -> Result<ModelHealthSnapshot, GatewayError> {
    let row: Option<(String, String)> = sqlx::query_as(
        "SELECT status, xmin::text || ':' || ctid::text
         FROM gateway_provider_credential_model_states WHERE id = $1",
    )
    .bind(identity.state_id())
    .fetch_optional(pool)
    .await
    .map_err(map_db_error)?;
    Ok(ModelHealthSnapshot {
        identity,
        protected: row.as_ref().is_some_and(|row| row.0 == "blocked"),
        version: row.map(|row| row.1),
    })
}

/// Lock and recheck the exact row before invoking the normal transition writer.
/// For an initially absent row, INSERT DO NOTHING prevents a concurrent failure overwrite.
pub async fn apply(
    pool: &PgPool,
    expected: &ModelHealthSnapshot,
    failure: Option<RecordCredentialModelFailureInput>,
) -> Result<ModelHealthRecovery, GatewayError> {
    if expected.protected {
        return Ok(ModelHealthRecovery::Protected);
    }
    let mut tx = pool.begin().await.map_err(map_db_error)?;
    let version: Option<String> = sqlx::query_scalar(
        "SELECT xmin::text || ':' || ctid::text FROM gateway_provider_credential_model_states
         WHERE id = $1 FOR UPDATE",
    )
    .bind(expected.identity.state_id())
    .fetch_optional(&mut *tx)
    .await
    .map_err(map_db_error)?;
    if version != expected.version {
        return Ok(ModelHealthRecovery::StaleHealth);
    }
    // Only the matching locked row may be overwritten. A missing row cannot be locked.
    let overwrite = version.is_some();
    let failed = failure.is_some();
    if failure.as_ref().is_some_and(|input| {
        !crate::provider_health_state::transition_for_failure_with_retry_after(
            &input.classification,
            input.upstream_retry_after_ms,
        )
        .penalizes_candidate
    }) {
        return Ok(ModelHealthRecovery::FailureNotPenalized);
    }
    let row = if let Some(mut input) = failure {
        // Identity comes only from the snapshot, not a second caller-controlled target.
        input.provider_account_id = expected.identity.provider_account_id.clone();
        input.provider_credential_id = expected.identity.provider_credential_id.clone();
        input.provider_credential_ref = expected.identity.provider_credential_ref.clone();
        input.protocol_profile = expected.identity.protocol_profile.clone();
        input.model = expected.identity.model.clone();
        super::write_failure(&mut *tx, input, overwrite).await?
    } else {
        super::write_success(&mut *tx, expected.identity.clone(), overwrite).await?
    };
    if row.is_none() {
        return Ok(ModelHealthRecovery::StaleHealth);
    }
    tx.commit().await.map_err(map_db_error)?;
    Ok(if failed {
        ModelHealthRecovery::FailureRecorded
    } else {
        ModelHealthRecovery::Applied
    })
}
