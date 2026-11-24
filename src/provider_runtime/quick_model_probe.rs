//! Exact-account console model probe for Amazon Quick (`quick-oauth`).

use super::{
    console_model_probe::ModelProbeResult, ProviderPayloadProbeReport, ProviderPayloadProbeStatus,
};
use crate::{protocol::quick_oauth, routing::config::CredentialProbeTarget, state::AppState};
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
    .unwrap_or_else(|_| failed(None, "Amazon Quick model test timed out after 60 seconds."));
    let health = super::model_probe_recording::finish(
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
                "Amazon Quick model {model}: {}",
                reply.chars().take(320).collect::<String>()
            ),
        ),
        Err(error) => (ProviderPayloadProbeStatus::Failed, error),
    };
    (
        format!("Amazon Quick model test ({model})"),
        ProviderPayloadProbeReport {
            status,
            message,
            answer,
            health_recovery: Some(health),
        },
    )
}

async fn generate(
    state: &AppState,
    target: &CredentialProbeTarget,
    model: &str,
    prompt: &str,
) -> ModelProbeResult {
    let request = match crate::protocol::openai::normalize_chat_completions(json!({
        "model": model,
        "stream": false,
        "max_tokens": 256,
        "n": 1,
        "messages": [{"role": "user", "content": prompt}]
    })) {
        Ok(request) => request,
        Err(_) => return failed(None, "Invalid Amazon Quick model test request."),
    };
    if state
        .route_config
        .snapshot()
        .model_probe_identity(target, model)
        .is_none()
    {
        return failed(
            None,
            "Amazon Quick credential binding changed; call not sent.",
        );
    }
    let response =
        quick_oauth::runtime::execute(&state.upstream_client, &target.payload, &request, model)
            .await;
    let mut response = match response {
        Ok(resp) => resp,
        Err(err) => {
            return failed(
                err.http_status,
                "Amazon Quick model test failed; verify OAuth credentials or upstream availability.",
            );
        }
    };
    if response.text.trim().is_empty() || response.text.chars().count() > 8192 {
        return failed(
            None,
            "Amazon Quick returned no complete non-empty text answer.",
        );
    }
    response.text = crate::error::sanitize_provider_error_message(
        &response.text.replace(&target.payload.api_key, "[redacted]"),
    );
    ModelProbeResult {
        status: None,
        reply: Ok(response.text),
        usage: response.usage.map_or(Value::Null, |u| {
            json!({
                "prompt_tokens": u.prompt_tokens,
                "completion_tokens": u.completion_tokens,
                "total_tokens": u.total_tokens,
                "cache_read_input_tokens": u.cache_read_input_tokens,
                "cache_creation_input_tokens": u.cache_creation_input_tokens,
            })
        }),
    }
}

fn failed(status: Option<u16>, message: &str) -> ModelProbeResult {
    ModelProbeResult {
        status,
        reply: Err(message.into()),
        usage: Value::Null,
    }
}
