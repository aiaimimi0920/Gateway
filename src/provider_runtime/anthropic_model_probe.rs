//! Explicit single-account Messages proof through the production dispatcher; no fallback.
use super::{
    console_model_probe::ModelProbeResult, ProviderPayloadProbeReport, ProviderPayloadProbeStatus,
};
use crate::protocol::accio::{parse_sse_line, ParsedEvent};
use crate::upstream::response_types::UpstreamStreamingResponse;
use crate::{routing::config::CredentialProbeTarget, state::AppState};
use futures::StreamExt;
use serde_json::{json, Value};
use std::time::{Duration, Instant};

pub(super) async fn probe(
    state: &AppState,
    target: &CredentialProbeTarget,
    model: &str,
    prompt: &str,
) -> (String, ProviderPayloadProbeReport) {
    let point = format!("Anthropic Messages model call (model: {model})");
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
    let request = crate::protocol::openai::normalize_chat_completions(
        json!({"model":model,"stream":true,"max_tokens":32,
        "messages":[{"role":"user","content":prompt}]}),
    );
    let Ok(request) = request else {
        return failed(None, "Cannot build the Anthropic model test request.");
    };
    // Keep production call-flow/transport and exact credentials, but require a real terminal
    // event rather than accepting the legacy accumulator's default stop reason on EOF.
    let response = match state
        .upstream_client
        .execute_stream_with_wire(&target.payload, &request, model, None, None)
        .await
    {
        Ok(response) => response,
        Err(error) => {
            return failed(
                error.http_status,
                &format!(
                    "Anthropic model request failed ({:?}, HTTP {}).",
                    error.kind,
                    error
                        .http_status
                        .map_or_else(|| "unknown".into(), |status| status.to_string())
                ),
            )
        }
    };
    type ProbeStream =
        std::pin::Pin<Box<dyn futures::Stream<Item = Result<bytes::Bytes, ()>> + Send>>;
    let (status, mut stream): (u16, ProbeStream) = match response {
        UpstreamStreamingResponse::Http(response) => (
            response.status().as_u16(),
            Box::pin(response.bytes_stream().map(|chunk| chunk.map_err(|_| ()))),
        ),
        UpstreamStreamingResponse::Native(response) => (
            response.status().as_u16(),
            Box::pin(response.bytes_stream().map(|chunk| chunk.map_err(|_| ()))),
        ),
        UpstreamStreamingResponse::NativeEvents(stream) => {
            (200, Box::pin(stream.map(|chunk| chunk.map_err(|_| ()))))
        }
        UpstreamStreamingResponse::Bytes(stream) => {
            (200, Box::pin(stream.map(|chunk| chunk.map_err(|_| ()))))
        }
    };
    let mut bytes = Vec::new();
    while let Some(chunk) = stream.next().await {
        match chunk {
            Ok(chunk) if bytes.len().saturating_add(chunk.len()) <= 262_144 => {
                bytes.extend_from_slice(&chunk)
            }
            Ok(_) => return failed(Some(status), "Model response exceeded 256 KiB."),
            Err(_) => return failed(Some(status), "Model response was interrupted."),
        }
    }
    let Ok(body) = std::str::from_utf8(&bytes) else {
        return failed(Some(status), "Invalid model response encoding.");
    };
    let mut reply = String::new();
    let (mut done, mut stopped) = (false, false);
    let (mut input, mut output) = (0u64, 0u64);
    for line in body.lines() {
        for event in parse_sse_line(line.as_bytes()).unwrap_or_default() {
            match event {
                ParsedEvent::Text(text) => reply.push_str(&text),
                ParsedEvent::Done => done = true,
                ParsedEvent::ProviderError { .. } => {
                    return failed(Some(status), "Upstream returned a stream error.")
                }
                ParsedEvent::Start { usage, .. } => {
                    if let Some(usage) = usage {
                        input = input.max(usage.prompt_tokens);
                        output = output.max(usage.completion_tokens);
                    }
                }
                ParsedEvent::Finish { reason, usage } => {
                    stopped = reason.as_deref() == Some("stop");
                    if let Some(usage) = usage {
                        input = input.max(usage.prompt_tokens);
                        output = output.max(usage.completion_tokens);
                    }
                }
                _ => {}
            }
        }
    }
    if !done || !stopped || reply.trim().is_empty() {
        return failed(
            Some(200),
            "No completed non-empty assistant reply was returned.",
        );
    }
    let usage = json!({"prompt_tokens":input,"completion_tokens":output,"total_tokens":input.saturating_add(output)});
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
