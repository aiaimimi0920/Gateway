//! Explicit single-account Web HTTP proof; no browser refresh, retry or routing fallback.
use super::{
    console_model_probe::ModelProbeResult, ProviderPayloadProbeReport, ProviderPayloadProbeStatus,
};
use crate::{error::GatewayError, routing::config::CredentialProbeTarget, state::AppState};
use serde_json::{json, Value};
use std::time::{Duration, Instant};

pub(super) async fn probe(
    state: &AppState,
    target: &CredentialProbeTarget,
    model: &str,
    prompt: &str,
) -> (String, ProviderPayloadProbeReport) {
    let point = format!("ChatGPT Web HTTP model call (model: {model})");
    let audit = super::model_probe_recording::begin(state, target, model, false).await;
    let started = Instant::now();
    let result = tokio::time::timeout(
        Duration::from_secs(60),
        generate(state, target, model, prompt),
    )
    .await
    .unwrap_or_else(|_| failed(None, "Model call timed out after 60 seconds.".into()));
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
        "model":model,"stream":false,"max_tokens":32,
        "messages":[{"role":"user","content":prompt}]
    }));
    let Ok(request) = request else {
        return failed(
            None,
            "Cannot build the ChatGPT Web model test request.".into(),
        );
    };
    // Reuse the production call-flow and HTTP accumulator, not the generic OpenAI planner
    // or pipeline recovery. An upstream rejection must not spawn a browser or change accounts.
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
        Err(error) => return failed(error.http_status, safe_failure(&error)),
    };
    if response.text.trim().is_empty() || response.finish_reason.as_deref() != Some("stop") {
        return failed(
            Some(200),
            "No completed non-empty assistant reply was returned.".into(),
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
    // Header redaction alone misses an echoed individual cookie value.
    for (_, cookie) in target
        .payload
        .headers
        .iter()
        .filter(|(name, _)| name.eq_ignore_ascii_case("cookie"))
    {
        for secret in cookie
            .split(';')
            .filter_map(|pair| pair.split_once('=').map(|(_, value)| value.trim()))
            .filter(|value| !value.is_empty())
        {
            reply = reply.replace(secret, "[redacted]");
        }
    }
    for field in ["accessToken", "sessionId", "deviceId", "cookieHeader"] {
        if let Some(secret) = target
            .payload
            .extra_body
            .as_ref()
            .and_then(|extra| extra.get(field))
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
        {
            reply = reply.replace(secret, "[redacted]");
        }
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

fn safe_failure(error: &GatewayError) -> String {
    // Error messages may contain upstream JSON or HTML. Only fixed stage labels and a
    // controlled error code/status cross the probe's evidence boundary.
    let stage = [
        "bootstrap",
        "requirements",
        "requirements_prepare",
        "requirements_finalize",
        "prepare",
        "conversation",
    ]
    .into_iter()
    .find(|stage| error.message.contains(&format!("stage={stage} ")))
    .unwrap_or("request");
    let code = error
        .code
        .as_deref()
        .filter(|code| {
            code.starts_with("chatgpt_web_")
                && code.len() <= 96
                && code
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b == b'_' || b.is_ascii_digit())
        })
        .unwrap_or("request_failed");
    format!(
        "ChatGPT Web HTTP request failed: stage={stage} code={code} HTTP {}.",
        error
            .http_status
            .map_or_else(|| "unknown".into(), |status| status.to_string())
    )
}

fn failed(status: Option<u16>, message: String) -> ModelProbeResult {
    ModelProbeResult {
        status,
        reply: Err(message),
        usage: Value::Null,
    }
}
