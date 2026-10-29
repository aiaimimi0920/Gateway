//! Use persistent local model health when ranking route candidates.
use super::LocalRuntime;
use crate::db::{GatewayProviderCredentialModelStateView, RecordCredentialModelSuccessInput};
use crate::provider_health_state::candidate_penalty_status;
use crate::{error::GatewayError, routing::candidate::RouteCandidate};

impl LocalRuntime {
    pub async fn apply_model_states(
        &self,
        candidates: &mut [RouteCandidate],
    ) -> Result<(), GatewayError> {
        for candidate in candidates {
            let Some(model) = candidate
                .upstream_model
                .as_deref()
                .or(candidate.model_alias.as_deref())
            else {
                continue;
            };
            let snapshot = self
                .model_health_snapshot(RecordCredentialModelSuccessInput {
                    provider_account_id: candidate.provider_account_id.clone(),
                    provider_credential_id: candidate.provider_credential_id.clone(),
                    provider_credential_ref: candidate
                        .payload
                        .credential_id
                        .clone()
                        .or_else(|| candidate.provider_credential_id.clone()),
                    protocol_profile: Some(candidate.protocol_profile.clone()),
                    model: model.to_owned(),
                })
                .await?;
            let Some(payload) = snapshot.version else {
                continue;
            };
            let state: GatewayProviderCredentialModelStateView = serde_json::from_str(&payload)
                .map_err(|_| GatewayError::server_error("Local model state is corrupt"))?;
            if let Some(cooldown) = &state.cooldown_until {
                if matches!(state.status.as_str(), "cooling" | "blocked") {
                    candidate.cooldown_until = Some(cooldown.clone());
                }
            } else if state.status == "blocked" {
                candidate.cooldown_until = Some("9999-12-31T23:59:59Z".into());
            }
            if let Some(reason) =
                candidate_penalty_status(&state.status, state.cooldown_until.as_deref())
            {
                candidate.routing_degradation_reasons.push(reason);
            }
            candidate.failure_count = candidate
                .failure_count
                .saturating_add(state.failure_count.max(0) as u32);
        }
        Ok(())
    }
}
