//! Exact-account SenseNova Token Plan text tests reuse cancellation-safe probe audit/health ownership.
use super::{
    console_model_probe::ModelProbeResult, model_probe_audit::InternalProbeProduct,
    model_probe_recording, ProviderPayloadProbeReport, ProviderPayloadProbeStatus,
};
use crate::{protocol::sensenova, routing::config::CredentialProbeTarget, state::AppState};
use serde_json::{json, Value};
use std::time::{Duration, Instant};

pub(super) async fn probe(
    state: &AppState,
    target: &CredentialProbeTarget,
    model: &str,
    prompt: &str,
) -> (String, ProviderPayloadProbeReport) {
    let point = format!("SenseNova Token Plan Chat test (model: {model})");
    let audit = match model_probe_recording::begin_internal(
        state,
        InternalProbeProduct::SenseNovaTokenPlan,
        target,
        model,
    )
    .await
    {
        Ok(audit) => audit,
        Err(failure) => return failure.report(point),
    };
    let started = Instant::now();
    let result = match tokio::time::timeout(Duration::from_secs(60),
        sensenova::probe::execute(&state.upstream_client, &target.payload, model, prompt)).await {
        Ok(Ok((status, response))) => ModelProbeResult {
            status: Some(status), reply: Ok(response.text),
            usage: response.usage.map_or(Value::Null, |u| json!({
                "prompt_tokens":u.prompt_tokens,"completion_tokens":u.completion_tokens,"total_tokens":u.total_tokens,
                "cache_read_input_tokens":u.cache_read_input_tokens,"cache_creation_input_tokens":u.cache_creation_input_tokens
            })),
        },
        // Never copy raw upstream error details or API keys into evidence.
        Ok(Err(_)) => failed("SenseNova Token Plan model test failed; invalid configuration, authentication or incomplete response."),
        Err(_) => failed("SenseNova Token Plan model test timed out after 60 seconds."),
    };
    let recovery = match model_probe_recording::finish_internal(
        state,
        target,
        model,
        audit,
        started.elapsed(),
        &result,
    )
    .await
    {
        Ok(recovery) => recovery,
        Err(failure) => return failure.report(point),
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
fn failed(message: &str) -> ModelProbeResult {
    ModelProbeResult {
        status: None,
        reply: Err(message.into()),
        usage: Value::Null,
    }
}
