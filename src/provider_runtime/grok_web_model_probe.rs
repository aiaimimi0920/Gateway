//! Explicit exact-account generation, using the same current WS owner and normal probe recording.
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
    .unwrap_or_else(|_| failed(None, "Grok Web model call timed out.".into()));
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
                "Grok Web model call passed; policy={model}. Reply: {}",
                reply.chars().take(320).collect::<String>()
            ),
        ),
        Err(message) => (ProviderPayloadProbeStatus::Failed, message),
    };
    (
        format!("Grok Web single-turn model call (policy: {model})"),
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
    let request = match crate::protocol::openai::normalize_chat_completions(json!({
        "model":model,"messages":[{"role":"user","content":prompt}],"stream":false
    })) {
        Ok(request) => request,
        Err(_) => return failed(None, "Cannot build Grok Web model test.".into()),
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
            let code = error
                .code
                .as_deref()
                .filter(|s| {
                    s.starts_with("grok_web_")
                        && s.len() <= 96
                        && s.bytes().all(|b| b.is_ascii_lowercase() || b == b'_')
                })
                .unwrap_or("request_failed");
            return failed(
                error.http_status,
                format!("Grok Web model call failed: {code}."),
            );
        }
    };
    if response.text.trim().is_empty() || response.finish_reason.as_deref() != Some("stop") {
        return failed(
            response.upstream_status,
            "No completed Grok Web answer was returned.".into(),
        );
    }
    let mut reply = response.text;
    let mut secrets: Vec<_> = std::iter::once(target.payload.api_key.as_str())
        .chain(target.payload.headers.values().map(String::as_str))
        .chain(
            target
                .payload
                .headers
                .iter()
                .filter(|(key, _)| key.eq_ignore_ascii_case("cookie"))
                .flat_map(|(_, value)| {
                    value
                        .split(';')
                        .filter_map(|pair| pair.split_once('=').map(|(_, v)| v.trim()))
                }),
        )
        .filter(|value| !value.is_empty())
        .collect();
    secrets.sort_unstable_by_key(|value| std::cmp::Reverse(value.len()));
    for secret in secrets {
        reply = reply.replace(secret, "[redacted]");
    }
    ModelProbeResult {
        status: response.upstream_status,
        usage: Value::Null,
        reply: Ok(crate::error::sanitize_provider_error_message(&reply)
            .chars()
            .take(8192)
            .collect()),
    }
}
fn failed(status: Option<u16>, message: String) -> ModelProbeResult {
    ModelProbeResult {
        status,
        reply: Err(message),
        usage: Value::Null,
    }
}
