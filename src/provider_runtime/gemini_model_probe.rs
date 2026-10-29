//! Exact-account Gemini prompt proof via the production accumulator, without routing fallback.
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
    let point = format!("Gemini API model call (model: {model})");
    let audit = super::model_probe_recording::begin(state, target, model, false).await;
    let started = Instant::now();
    let result = tokio::time::timeout(
        Duration::from_secs(60),
        generate(state, target, model, prompt),
    )
    .await
    .unwrap_or_else(|_| failed(None, "Model call timed out after 60 seconds."));
    let health_recovery = super::model_probe_recording::finish(
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
        Err(error) => (
            ProviderPayloadProbeStatus::Failed,
            format!("Model call failed. Model: {model}. {error}"),
        ),
    };
    (
        point,
        ProviderPayloadProbeReport {
            health_recovery: Some(health_recovery),
            status,
            message,
            answer,
        },
    )
}

async fn generate(
    state: &AppState,
    target: &CredentialProbeTarget,
    model: &str,
    prompt: &str,
) -> ModelProbeResult {
    let request = crate::protocol::openai::normalize_chat_completions(json!({
        "model": model, "stream": false, "max_tokens": 256,
        "messages": [{"role": "user", "content": prompt}]
    }));
    let Ok(request) = request else {
        return failed(None, "Cannot build the Gemini model test request.");
    };
    // Production Gemini uses forced-stream accumulation even for a non-streaming request.
    // This proves completed output, not token-by-token downstream forwarding.
    let response = match state
        .upstream_client
        .execute(&target.payload, &request, model, None)
        .await
    {
        Ok(response) => response,
        Err(error) => {
            return failed(
                error.http_status,
                &format!(
                    "Gemini model request failed ({:?}, HTTP {}).",
                    error.kind,
                    error
                        .http_status
                        .map_or_else(|| "unknown".into(), |status| status.to_string())
                ),
            )
        }
    };
    if response.text.trim().is_empty() || response.finish_reason.as_deref() != Some("stop") {
        return failed(
            Some(200),
            "No completed non-empty assistant reply was returned.",
        );
    }
    let usage = response.usage.map_or(Value::Null, |usage| json!({
        "prompt_tokens":usage.prompt_tokens,"completion_tokens":usage.completion_tokens,"total_tokens":usage.total_tokens
    }));
    let mut reply = response.text;
    for secret in std::iter::once(target.payload.api_key.as_str())
        .chain(target.payload.auth_token.as_deref())
        .chain(target.payload.headers.values().map(String::as_str))
        .filter(|value| !value.is_empty())
    {
        reply = reply.replace(secret, "[redacted]");
    }
    ModelProbeResult {
        status: Some(200),
        usage,
        reply: Ok(crate::error::sanitize_provider_error_message(&reply)
            .chars()
            .take(8192)
            .collect()),
    }
}

fn failed(status: Option<u16>, message: &str) -> ModelProbeResult {
    ModelProbeResult {
        status,
        reply: Err(message.into()),
        usage: Value::Null,
    }
}
