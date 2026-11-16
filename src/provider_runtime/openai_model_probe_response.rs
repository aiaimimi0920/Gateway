//! Legacy generic OpenAI console response handling; product probes own stricter wire contracts.
use super::console_model_probe::ModelProbeResult;
use crate::{routing::config::CredentialProbeTarget, state::AppState};
use futures::StreamExt;
use serde_json::Value;
use std::time::Duration;

pub(super) async fn generate(
    state: &AppState,
    target: &CredentialProbeTarget,
    plan: &crate::upstream::common::RequestPlan,
) -> ModelProbeResult {
    let request = crate::upstream::common::build_request_builder_from_plan(
        state.upstream_client.client(),
        Duration::from_secs(60),
        plan,
        crate::upstream::headers::build_upstream_headers(&target.payload),
    );
    let response = match request.send().await {
        Ok(response) => response,
        Err(_) => return failed(None, "Cannot reach the NVIDIA model endpoint."),
    };
    let status = response.status().as_u16();
    if !response.status().is_success() {
        let reason = match status {
            401 => "Credential authentication rejected",
            403 => "Credential has no access to this model",
            404 => "Configured model or generation endpoint was not found",
            429 => "Upstream rate limit or quota exhausted",
            _ => "Upstream model request failed",
        };
        return failed(Some(status), &format!("HTTP {status}: {reason}."));
    }
    // Bound the complete response, not just the preview; never log upstream bodies or keys.
    let stream = response.bytes_stream();
    futures::pin_mut!(stream);
    let mut bytes = Vec::new();
    loop {
        match stream.next().await {
            Some(Ok(chunk)) if bytes.len() + chunk.len() <= 65_536 => {
                bytes.extend_from_slice(&chunk)
            }
            Some(Ok(_)) => return failed(Some(status), "Model response exceeded 64 KiB."),
            None => break,
            Some(Err(_)) => return failed(Some(status), "Model response was interrupted."),
        }
    }
    let Ok(body) = serde_json::from_slice::<Value>(&bytes) else {
        return failed(Some(status), "Model endpoint returned invalid JSON.");
    };
    let content = body
        .pointer("/choices/0/message/content")
        .and_then(Value::as_str)
        .map(str::trim);
    if body.get("error").is_some_and(|value| !value.is_null()) || content.is_none_or(str::is_empty)
    {
        return failed(
            Some(status),
            "HTTP succeeded but no non-empty assistant reply was returned.",
        );
    }
    if body
        .pointer("/choices/0/finish_reason")
        .and_then(Value::as_str)
        == Some("length")
    {
        return failed(
            Some(status),
            "Model hit the test token limit before completing its reply.",
        );
    }
    let mut reply = content.unwrap_or_default().to_string();
    for secret in std::iter::once(target.payload.api_key.as_str())
        .chain(target.payload.auth_token.as_deref())
        .chain(target.payload.headers.values().map(String::as_str))
        .filter(|value| !value.is_empty())
    {
        reply = reply.replace(secret, "[redacted]");
    }
    let reply = crate::error::sanitize_provider_error_message(&reply)
        .chars()
        .take(8192)
        .collect();
    ModelProbeResult {
        status: Some(status),
        reply: Ok(reply),
        usage: body.get("usage").cloned().unwrap_or(Value::Null),
    }
}

fn failed(status: Option<u16>, message: &str) -> ModelProbeResult {
    ModelProbeResult {
        status,
        reply: Err(message.into()),
        usage: Value::Null,
    }
}
