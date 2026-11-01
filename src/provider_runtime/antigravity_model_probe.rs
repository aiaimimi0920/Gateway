//! Explicit, exact-account generation through the production HTTP owner, without retries.
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
    let audit = super::model_probe_recording::begin(state, target, model, false).await;
    let started = Instant::now();
    let result = tokio::time::timeout(
        Duration::from_secs(60),
        generate(state, target, model, prompt),
    )
    .await
    .unwrap_or_else(|_| failed(None, "Antigravity model test timed out; not replayed"));
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
                "Antigravity model call passed. Model: {model}. Reply: {}",
                reply.chars().take(320).collect::<String>()
            ),
        ),
        Err(error) => (
            ProviderPayloadProbeStatus::Failed,
            format!("Antigravity model call failed. Model: {model}. {error}"),
        ),
    };
    (
        format!("Antigravity Cloud Code HTTP (model: {model})"),
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
    let request = crate::protocol::openai::normalize_chat_completions(
        json!({"model":model,"stream":false,"max_tokens":256,
        "messages":[{"role":"user","content":prompt}]}),
    );
    let Ok(request) = request else {
        return failed(None, "Cannot build Antigravity prompt test");
    };
    let response = match state
        .upstream_client
        .execute_with_provider_account_id(
            &target.provider_id,
            &target.payload,
            &request,
            model,
            None,
        )
        .await
    {
        Ok(response) => response,
        Err(error) => {
            return failed(
                error.http_status,
                "Antigravity HTTP generation failed; private upstream response omitted",
            )
        }
    };
    if response.text.trim().is_empty() || response.finish_reason.as_deref() != Some("stop") {
        return failed(
            Some(200),
            "No completed non-empty assistant reply was returned",
        );
    }
    let usage=response.usage.map_or(Value::Null,|u|json!({"prompt_tokens":u.prompt_tokens,"completion_tokens":u.completion_tokens,"total_tokens":u.total_tokens}));
    let reply = response.text.replace(&target.payload.api_key, "[redacted]");
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
        usage: Value::Null,
        reply: Err(message.into()),
    }
}
