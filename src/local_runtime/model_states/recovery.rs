//! Recovery shares the normal local transaction writer; no parallel health store.
use super::*;
use crate::db::provider_credential_model_states::recovery::{
    ModelHealthRecovery, ModelHealthSnapshot,
};

impl LocalRuntime {
    pub async fn model_health_snapshot(
        &self,
        identity: RecordCredentialModelSuccessInput,
    ) -> Result<ModelHealthSnapshot, GatewayError> {
        let version: Option<String> =
            sqlx::query_scalar("SELECT payload FROM credential_model_states WHERE id = ?")
                .bind(identity.state_id())
                .fetch_optional(&self.pool)
                .await
                .map_err(storage_error)?;
        let protected = version
            .as_deref()
            .map(decode)
            .transpose()?
            .is_some_and(|row| row.status == "blocked");
        Ok(ModelHealthSnapshot {
            identity,
            version,
            protected,
        })
    }

    pub async fn apply_model_health_snapshot(
        &self,
        expected: &ModelHealthSnapshot,
        failure: Option<RecordCredentialModelFailureInput>,
    ) -> Result<ModelHealthRecovery, GatewayError> {
        if expected.protected {
            return Ok(ModelHealthRecovery::Protected);
        }
        if failure.as_ref().is_some_and(|input| {
            !transition_for_failure_with_retry_after(
                &input.classification,
                input.upstream_retry_after_ms,
            )
            .penalizes_candidate
        }) {
            return Ok(ModelHealthRecovery::FailureNotPenalized);
        }
        let failed = failure.is_some();
        let failure = failure.map(|mut input| {
            input.provider_account_id = expected.identity.provider_account_id.clone();
            input.provider_credential_id = expected.identity.provider_credential_id.clone();
            input.provider_credential_ref = expected.identity.provider_credential_ref.clone();
            input.protocol_profile = expected.identity.protocol_profile.clone();
            input.model = expected.identity.model.clone();
            input
        });
        let row = self
            .update_model(expected.identity.clone(), failure, Some(expected))
            .await?;
        Ok(if row.is_none() {
            ModelHealthRecovery::StaleHealth
        } else if failed {
            ModelHealthRecovery::FailureRecorded
        } else {
            ModelHealthRecovery::Applied
        })
    }
}
