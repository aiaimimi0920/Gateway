//! Explicit exact-account Chat tests use the normal sender and existing audit/health owner.
use super::{
    console_model_probe::ModelProbeResult, ProviderPayloadProbeReport, ProviderPayloadProbeStatus,
};
use crate::{routing::config::CredentialProbeTarget, state::AppState};
use serde_json::{json, Value};
use std::time::{Duration, Instant};

pub(super) async fn probe(
    state: &AppState,
    target: &CredentialProbeTarget,
    model: &str,
    prompt: &str,
) -> (String, ProviderPayloadProbeReport) {
    let point = format!("Luma inference Chat test (model: {model})");
    let audit = super::model_probe_recording::begin(state, target, model, false).await;
    let started = Instant::now();
    let result = match tokio::time::timeout(Duration::from_secs(60), execute(state, target, model, prompt)).await {
        Ok(Ok(response)) => ModelProbeResult { status: Some(200), reply: Ok(response.text),
            usage: response.usage.map_or(Value::Null, |u| json!({"prompt_tokens":u.prompt_tokens,
                "completion_tokens":u.completion_tokens,"total_tokens":u.total_tokens,
                "cache_read_input_tokens":u.cache_read_input_tokens,"cache_creation_input_tokens":u.cache_creation_input_tokens})) },
        Ok(Err(_)) => ModelProbeResult { status: None, usage: Value::Null, reply: Err("Luma inference model test failed; invalid configuration, authentication or response.".into()) },
        Err(_) => ModelProbeResult { status: None, usage: Value::Null, reply: Err("Luma inference model test timed out after 60 seconds.".into()) },
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
async fn execute(
    state: &AppState,
    target: &CredentialProbeTarget,
    model: &str,
    prompt: &str,
) -> Result<crate::protocol::canonical::CanonicalRelayResponse, crate::error::GatewayError> {
    let request =
        crate::protocol::openai::normalize_chat_completions(json!({"model":model,"stream":false,
        "max_tokens":256,"n":1,"messages":[{"role":"user","content":prompt}]}))?;
    let raw = state
        .upstream_client
        .execute_json_passthrough(
            &target.provider_id,
            crate::routing::candidate::ProviderExecutionMode::DirectHttp,
            &target.payload,
            &request,
            model,
            None,
        )
        .await?;
    if raw["choices"].as_array().is_some_and(|choices| {
        choices.iter().any(|choice| {
            choice["message"]
                .get("refusal")
                .is_some_and(|v| !v.is_null())
        })
    }) {
        return Err(crate::protocol::luma_inference::response_error());
    }
    let mut response = crate::protocol::openai::chat_admission::unpack(&raw)
        .map_err(|_| crate::protocol::luma_inference::response_error())?;
    if response.finish_reason.as_deref() != Some("stop")
        || !response.tool_calls.is_empty()
        || response.text.trim().is_empty()
        || response.text.chars().count() > 8192
    {
        return Err(crate::protocol::luma_inference::response_error());
    }
    response.text = crate::error::sanitize_provider_error_message(
        &response.text.replace(&target.payload.api_key, "[redacted]"),
    );
    Ok(response)
}
