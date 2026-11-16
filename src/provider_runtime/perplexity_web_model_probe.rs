//! Opt-in exact-account generation through the ordinary Web dispatcher, never generic API discovery.
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
    let point = format!("Perplexity Web single-turn model call (policy: {model})");
    if model != "turbo" {
        return (
            point,
            ProviderPayloadProbeReport {
                health_recovery: None,
                status: ProviderPayloadProbeStatus::Unsupported,
                message:
                    "Perplexity Web currently supports only the pinned turbo policy; call not sent."
                        .into(),
                answer: None,
            },
        );
    }
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
            "Perplexity Web model call timed out after 60 seconds.".into(),
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
                "Perplexity Web model call passed. Policy: {model}. Reply: {}",
                reply.chars().take(320).collect::<String>()
            ),
        ),
        Err(error) => (ProviderPayloadProbeStatus::Failed, error),
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
    // No unsupported max_tokens/defaults, no route selection, recovery or account/channel fallback.
    let request = crate::protocol::openai::normalize_chat_completions(json!({
        "model":model,"stream":false,"messages":[{"role":"user","content":prompt}]
    }));
    let Ok(request) = request else {
        return failed(
            None,
            "Cannot build the Perplexity Web model test request.".into(),
        );
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
            // Neither network details nor untrusted error text cross the evidence boundary.
            let code = error
                .code
                .as_deref()
                .filter(|code| {
                    code.starts_with("perplexity_web_")
                        && code.len() <= 96
                        && code
                            .bytes()
                            .all(|b| b.is_ascii_lowercase() || b == b'_' || b.is_ascii_digit())
                })
                .unwrap_or("request_failed");
            return failed(
                error.http_status,
                format!("Perplexity Web request failed: code={code}."),
            );
        }
    };
    if response.text.trim().is_empty() || response.finish_reason.as_deref() != Some("stop") {
        return failed(
            response.upstream_status,
            "No completed non-empty Perplexity Web answer was returned.".into(),
        );
    }
    let mut reply = response.text;
    for secret in std::iter::once(target.payload.api_key.as_str())
        .chain(target.payload.headers.values().map(String::as_str))
        .filter(|v| !v.is_empty())
    {
        reply = reply.replace(secret, "[redacted]");
    }
    for (_, cookie) in target
        .payload
        .headers
        .iter()
        .filter(|(key, _)| key.eq_ignore_ascii_case("cookie"))
    {
        for secret in cookie
            .split(';')
            .filter_map(|pair| pair.split_once('=').map(|(_, value)| value.trim()))
            .filter(|v| !v.is_empty())
        {
            reply = reply.replace(secret, "[redacted]");
        }
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
