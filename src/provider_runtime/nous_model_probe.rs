//! Exact-account Nous Portal text tests reuse cancellation-safe probe audit/health ownership.
use super::{
    console_model_probe::ModelProbeResult, ProviderPayloadProbeReport, ProviderPayloadProbeStatus,
};
use crate::{protocol::nous_portal, routing::config::CredentialProbeTarget, state::AppState};
use serde_json::{json, Value};
use std::time::{Duration, Instant};

pub(super) async fn probe(
    state: &AppState,
    target: &CredentialProbeTarget,
    model: &str,
    prompt: &str,
) -> (String, ProviderPayloadProbeReport) {
    let point = format!("Nous Portal Chat test (model: {model})");
    let audit = super::model_probe_recording::begin(state, target, model, false).await;
    let started = Instant::now();
    // Audit setup yields. Recheck the selected credential after it, without silently
    // switching a paid target or creating another refresh owner. In-flight sends stay pinned.
    let result = if state
        .route_config
        .snapshot()
        .model_probe_identity(target, model)
        .is_none()
    {
        failed("Nous Portal credential binding changed; call not sent. Select the current account again.")
    } else {
        match tokio::time::timeout(Duration::from_secs(60),
        nous_portal::probe::execute(&state.upstream_client, &target.payload, model, prompt)).await {
        Ok(Ok((status, response))) => ModelProbeResult {
            status: Some(status), reply: Ok(response.text),
            usage: response.usage.map_or(Value::Null, |u| json!({
                "prompt_tokens":u.prompt_tokens,"completion_tokens":u.completion_tokens,"total_tokens":u.total_tokens,
                "cache_read_input_tokens":u.cache_read_input_tokens,"cache_creation_input_tokens":u.cache_creation_input_tokens
            })),
        },
        // Never copy raw upstream error details or API keys into evidence.
        Ok(Err(_)) => failed("Nous Portal model test failed; invalid configuration, authentication or incomplete response."),
        Err(_) => failed("Nous Portal model test timed out after 60 seconds."),
    }
    };
    let recovery = super::model_probe_recording::finish(
        state,
        target,
        model,
        audit,
        started.elapsed(),
        &result,
    )
    .await;
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
