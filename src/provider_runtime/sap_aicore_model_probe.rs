//! Exact-account SAP text tests reuse cancellation-safe probe audit/health ownership.
use super::{
    console_model_probe::ModelProbeResult, ProviderPayloadProbeReport, ProviderPayloadProbeStatus,
};
use crate::{protocol::sap_aicore, routing::config::CredentialProbeTarget, state::AppState};
use serde_json::{json, Value};
use std::time::{Duration, Instant};

pub(super) async fn probe(
    state: &AppState,
    target: &CredentialProbeTarget,
    model: &str,
    prompt: &str,
) -> (String, ProviderPayloadProbeReport) {
    let point = format!("SAP AI Core Chat deployment test (model: {model})");
    let evidence = sap_aicore::probe_evidence::ProbeEvidence::new(
        &target.provider_id,
        &target.credential_id,
        model,
    );
    let audit = match super::model_probe_recording::sap::begin(
        state,
        target,
        model,
        evidence.clone(),
    )
    .await
    {
        Ok(audit) => audit,
        Err(error) => return error.report(point),
    };
    let started = Instant::now();
    let mut failure = None;
    let result = match tokio::time::timeout(Duration::from_secs(60),
        sap_aicore::probe::execute(&state.upstream_client, &target.payload, model, prompt, &evidence)).await {
        Ok(Ok((status, response))) => ModelProbeResult {
            status: Some(status), reply: Ok(response.text),
            usage: response.usage.map_or(Value::Null, |u| json!({
                "prompt_tokens":u.prompt_tokens,"completion_tokens":u.completion_tokens,"total_tokens":u.total_tokens,
                "cache_read_input_tokens":u.cache_read_input_tokens,"cache_creation_input_tokens":u.cache_creation_input_tokens
            })),
        },
        // Never copy raw upstream error details or service material into evidence.
        Ok(Err(error)) => {
            evidence.fail(&error);
            let result = failed(evidence.status(), &error.message);
            failure = Some(error);
            result
        },
        Err(_) => {
            let error = sap_aicore::failure("sap_aicore_probe_timeout");
            evidence.fail(&error);
            failure = Some(error);
            failed(evidence.status(), "SAP AI Core model test timed out after 60 seconds.")
        },
    };
    let recovery = match super::model_probe_recording::sap::finish(
        state,
        target,
        model,
        audit,
        started.elapsed(),
        &result,
        failure.as_ref(),
    )
    .await
    {
        Ok(recovery) => recovery,
        Err(error) => return error.report(point),
    };
    let answer = result.reply.as_ref().ok().cloned();
    let (status, message) = match result.reply {
        Ok(reply) => (
            ProviderPayloadProbeStatus::Passed,
            format!(
                "Model call passed. Model: {model}. Reply: {}",
                reply.chars().take(320).collect::<String>()
            ),
        ),
        Err(error) => (ProviderPayloadProbeStatus::Failed, error),
    };
    (
        point,
        ProviderPayloadProbeReport {
            health_recovery: Some(recovery),
            status,
            message,
            answer,
        },
    )
}
fn failed(status: Option<u16>, message: &str) -> ModelProbeResult {
    ModelProbeResult {
        status,
        reply: Err(message.into()),
        usage: Value::Null,
    }
}
