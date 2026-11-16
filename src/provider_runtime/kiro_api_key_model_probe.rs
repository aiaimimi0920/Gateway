//! Explicit CLI runtime generation uses only the selected Key/OAuth credential and ordinary sender.
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
    let product = product(target);
    let point = format!("{product} model call (model: {model})");
    let audit = super::model_probe_recording::begin(state, target, model, false).await;
    let started = Instant::now();
    let result = tokio::time::timeout(
        Duration::from_secs(60),
        generate(state, target, model, prompt),
    )
    .await
    .unwrap_or_else(|_| {
        failed(
            None,
            &format!("{product} model call timed out after 60 seconds."),
        )
    });
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
                "{product} model call passed. Model: {model}. Reply: {}",
                reply.chars().take(320).collect::<String>()
            ),
        ),
        Err(error) => (
            ProviderPayloadProbeStatus::Failed,
            format!("{product} model call failed. Model: {model}. {error}"),
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
    let product = product(target);
    let request =
        crate::protocol::openai::normalize_chat_completions(json!({"model":model,"stream":false,
        "messages":[{"role":"user","content":prompt}]}));
    let Ok(request) = request else {
        return failed(None, &format!("Cannot build {product} model test."));
    };
    // No routing/fallback, token refresh, implicit model discovery or second generation attempt.
    let result = state
        .upstream_client
        .execute_with_provider_account_id(
            &target.provider_id,
            &target.payload,
            &request,
            model,
            None,
        )
        .await;
    let response = match result {
        Ok(response) => response,
        Err(error) => {
            return failed(
                error.http_status,
                &format!(
                    "{product} request failed ({:?}, HTTP {}).",
                    error.kind,
                    error
                        .http_status
                        .map_or_else(|| "unknown".into(), |s| s.to_string())
                ),
            )
        }
    };
    if response.finish_reason.as_deref() != Some("stop")
        || !response.tool_calls.is_empty()
        || response.text.trim().is_empty()
    {
        return failed(
            response.upstream_status,
            &format!("No completed non-empty text-only {product} reply was returned."),
        );
    }
    if response.text.chars().count() > 8192 {
        return failed(
            response.upstream_status,
            &format!("{product} test reply exceeded 8192 characters."),
        );
    }
    let reply = response.text.replace(&target.payload.api_key, "[redacted]");
    ModelProbeResult{status:response.upstream_status,reply:Ok(crate::error::sanitize_provider_error_message(&reply)),
        usage:response.usage.map_or(Value::Null,|u|json!({"prompt_tokens":u.prompt_tokens,"completion_tokens":u.completion_tokens,"total_tokens":u.total_tokens}))}
}
fn failed(status: Option<u16>, message: &str) -> ModelProbeResult {
    ModelProbeResult {
        status,
        usage: Value::Null,
        reply: Err(message.into()),
    }
}
fn product(target: &CredentialProbeTarget) -> &'static str {
    if crate::protocol::kiro_oauth::runtime::owns_payload(&target.payload) {
        "Kiro OAuth runtime"
    } else {
        "Kiro CLI Key"
    }
}
