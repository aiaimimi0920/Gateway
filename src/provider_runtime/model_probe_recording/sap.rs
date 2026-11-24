//! SAP probe audit retains real send facts through cancellation, without generic result expansion.
use super::*;
use crate::protocol::sap_aicore::probe_evidence::ProbeEvidence;

pub(in crate::provider_runtime) async fn begin(
    state: &AppState,
    target: &CredentialProbeTarget,
    model: &str,
    evidence: ProbeEvidence,
) -> Result<Recording, InternalAuditFailure> {
    let health = super::super::model_probe_health::begin(state, target, model).await;
    let trace = json!({"consoleModelProbe":true,"routeAttemptCount":0,"realCredentialRef":null,
        "sapProbe":evidence.snapshot()});
    let mut input = audit_input(
        target,
        model,
        model,
        ("openai", "chat_completions", false),
        &trace,
    );
    input.route_attempt_count = 0;
    let mut audit = if requires_server_audit(state) {
        // Internal rows identify the configured probe target, not a public caller or a debit.
        ProbeAudit::begin_server_internal(
            state,
            InternalProbeProduct::SapAiCore,
            &target.credential_id,
            input,
        )
        .await
        .map_err(|_| InternalAuditFailure::Admission)?
    } else {
        // Local finalization cannot clear a prefilled account. Only an actual send projects it.
        input.provider_account_id = None;
        input.resolved_model = None;
        ProbeAudit::begin(state, input)
            .await
            .ok_or(InternalAuditFailure::Admission)?
    };
    audit.attach_sap(evidence.clone());
    Ok(Recording {
        audit: Some(audit),
        health,
        trace,
        response_id: None,
        sap: Some(evidence),
    })
}

pub(in crate::provider_runtime) async fn finish(
    state: &AppState,
    target: &CredentialProbeTarget,
    model: &str,
    recording: Recording,
    elapsed: Duration,
    result: &ModelProbeResult,
    error: Option<&GatewayError>,
) -> Result<db::provider_credential_model_states::recovery::ModelHealthRecovery, InternalAuditFailure>
{
    let (health, stored) = finish_audit(target, model, recording, elapsed, result).await;
    stored.map_err(|_| InternalAuditFailure::Finalization)?;
    Ok(super::super::model_probe_health::finish_classified(
        state, target, model, health, result, error,
    )
    .await)
}
