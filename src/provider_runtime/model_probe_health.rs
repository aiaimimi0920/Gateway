//! Binding checks and expected-row CAS, separate from generation/audit success.
use super::console_model_probe::ModelProbeResult;
use crate::{
    db::{
        self,
        provider_credential_model_states::recovery::{
            self, ModelHealthRecovery, ModelHealthSnapshot,
        },
    },
    routing::config::CredentialProbeTarget,
    state::AppState,
};

pub(super) enum ProbeHealth {
    Ready {
        revision: String,
        expected: ModelHealthSnapshot,
    },
    Skipped(ModelHealthRecovery),
}

pub(super) async fn begin(
    state: &AppState,
    target: &CredentialProbeTarget,
    model: &str,
) -> ProbeHealth {
    let route = state.route_config.snapshot();
    let Some(identity) = route.model_probe_identity(target, model) else {
        return ProbeHealth::Skipped(ModelHealthRecovery::StaleConfiguration);
    };
    let result = if let Some(local) = &state.local_runtime {
        local.model_health_snapshot(identity).await
    } else if let Some(pool) = &state.pg_pool {
        recovery::snapshot(pool, identity).await
    } else {
        return ProbeHealth::Skipped(ModelHealthRecovery::Unavailable);
    };
    match result {
        Ok(expected) => ProbeHealth::Ready {
            revision: route.revision().id().into(),
            expected,
        },
        Err(error) => {
            tracing::warn!(code = ?error.code, "cannot read model probe health snapshot");
            ProbeHealth::Skipped(ModelHealthRecovery::StorageFailed)
        }
    }
}

pub(super) async fn finish(
    state: &AppState,
    target: &CredentialProbeTarget,
    model: &str,
    health: ProbeHealth,
    result: &ModelProbeResult,
) -> ModelHealthRecovery {
    let ProbeHealth::Ready { revision, expected } = health else {
        let ProbeHealth::Skipped(outcome) = health else {
            unreachable!()
        };
        return outcome;
    };
    let matches = || {
        let route = state.route_config.snapshot();
        route.revision().id() == revision
            && route
                .model_probe_identity(target, model)
                .is_some_and(|identity| identity.state_id() == expected.identity.state_id())
    };
    if !matches() {
        return ModelHealthRecovery::StaleConfiguration;
    }
    let failure =
        result
            .reply
            .as_ref()
            .err()
            .map(|message| db::RecordCredentialModelFailureInput {
                provider_account_id: expected.identity.provider_account_id.clone(),
                provider_credential_id: expected.identity.provider_credential_id.clone(),
                provider_credential_ref: expected.identity.provider_credential_ref.clone(),
                protocol_profile: expected.identity.protocol_profile.clone(),
                model: model.into(),
                upstream_status: result.status,
                upstream_retry_after_ms: None,
                error_message: Some(message.clone()),
                classification: crate::provider_failure::classify_provider_failure(
                    result.status,
                    None,
                    Some(message),
                ),
            });
    let update = if let Some(local) = &state.local_runtime {
        local.apply_model_health_snapshot(&expected, failure).await
    } else if let Some(pool) = &state.pg_pool {
        recovery::apply(pool, &expected, failure).await
    } else {
        return ModelHealthRecovery::Unavailable;
    };
    match update {
        Ok(outcome) => {
            // Route publication and SQL have separate owners. Never claim cross-store atomicity
            // or compensate a late route change by overwriting newer health with an old snapshot.
            if matches!(
                outcome,
                ModelHealthRecovery::Applied | ModelHealthRecovery::FailureRecorded
            ) && !matches()
            {
                ModelHealthRecovery::ConfigurationChangedAfterWrite
            } else {
                outcome
            }
        }
        Err(error) => {
            tracing::warn!(code = ?error.code, "cannot confirm model probe health update");
            ModelHealthRecovery::StorageFailed
        }
    }
}
