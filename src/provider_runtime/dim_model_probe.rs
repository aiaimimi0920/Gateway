//! Keep the original target for health identity; only the wire payload is request-local.
use super::{
    console_model_probe::ModelProbeResult, ProviderPayloadProbeReport, ProviderPayloadProbeStatus,
};
use crate::{
    routing::config::CredentialProbeTarget, state::AppState, upstream::client::UpstreamClient,
};
use futures::StreamExt;
use serde_json::{json, Value};
use std::time::{Duration, Instant};

pub(super) async fn probe(
    state: &AppState,
    target: &CredentialProbeTarget,
    model: &str,
    prompt: &str,
) -> (String, ProviderPayloadProbeReport) {
    let payload = state
        .route_config
        .snapshot()
        .get_providers()
        .into_iter()
        .find(|p| p.id == target.provider_id)
        .and_then(|p| {
            p.credential_pool
                .into_iter()
                .find(|c| c.id == target.credential_id)
        })
        .and_then(|c| c.dim_catalog)
        .and_then(|c| c.project(&target.payload, model).ok());
    let Some((payload, family)) = payload else {
        return (
            "DIM model call".into(),
            ProviderPayloadProbeReport {
                status: ProviderPayloadProbeStatus::Unsupported,
                message:
                    "DIM requires a saved, account-bound wire declaration; no request was sent."
                        .into(),
                answer: None,
                health_recovery: None,
            },
        );
    };
    let audit =
        super::model_probe_recording::begin(state, target, model, family == "openai_responses")
            .await;
    let started = Instant::now();
    let result = tokio::time::timeout(
        Duration::from_secs(60),
        generate(state, &payload, model, prompt, family),
    )
    .await
    .unwrap_or_else(|_| failed(None, "DIM model call timed out."));
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
                "DIM {family} model {model}: {}",
                reply.chars().take(320).collect::<String>()
            ),
        ),
        Err(error) => (ProviderPayloadProbeStatus::Failed, error),
    };
    (
        format!("DIM {family} model call"),
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
    payload: &crate::routing::candidate::ProviderAccountPayload,
    model: &str,
    prompt: &str,
    family: &str,
) -> ModelProbeResult {
    let request = crate::protocol::openai::normalize_chat_completions(
        json!({"model":model,"stream":true,"max_tokens":256,"messages":[{"role":"user","content":prompt}]}),
    );
    let Ok(request) = request else {
        return failed(None, "Invalid DIM test request.");
    };
    // Inspect the actual declared wire, not the generic Gemini-to-OpenAI aggregated bridge.
    let plan = match UpstreamClient::build_request_plan(payload, &request, model, true) {
        Ok(plan) => plan,
        Err(_) => return failed(None, "DIM model request could not be prepared."),
    };
    let pending = match state.upstream_client.send_profiled_plan(
        payload,
        &plan,
        crate::upstream::headers::build_upstream_headers(payload),
    ) {
        Ok(pending) => pending,
        Err(_) => return failed(None, "DIM model request could not be admitted."),
    };
    let response = match pending.send().await {
        Ok(response) => response,
        Err(error) => return failed(error.http_status, "DIM upstream model request failed."),
    };
    if !response.status().is_success() {
        return failed(
            Some(response.status().as_u16()),
            "DIM upstream rejected the model request.",
        );
    }
    let mut stream = response.bytes_stream();
    let mut bytes = Vec::new();
    while let Some(chunk) = stream.next().await {
        match chunk {
            Ok(chunk) if bytes.len().saturating_add(chunk.len()) <= 262144 => {
                bytes.extend_from_slice(&chunk)
            }
            _ => {
                return failed(
                    None,
                    "DIM test response was interrupted or exceeded 256 KiB.",
                )
            }
        }
    }
    let Ok(body) = std::str::from_utf8(&bytes) else {
        return failed(None, "Invalid DIM response encoding.");
    };
    match parse(body, family) {
        Some(reply) => ModelProbeResult {
            status: Some(200),
            usage: Value::Null,
            reply: Ok(crate::error::sanitize_provider_error_message(
                &reply.replace(&payload.api_key, "[redacted]"),
            )),
        },
        None => failed(Some(200), "DIM returned no completed non-empty text reply."),
    }
}
fn parse(body: &str, family: &str) -> Option<String> {
    let (mut text, mut done, mut stop) = (String::new(), false, false);
    for line in body
        .lines()
        .filter_map(|l| l.strip_prefix("data:").map(str::trim))
    {
        if line == "[DONE]" {
            done = true;
            continue;
        }
        let value: Value = serde_json::from_str(line).ok()?;
        if value.get("error").is_some()
            || value["type"] == "error"
            || matches!(
                value["type"].as_str(),
                Some("response.failed" | "response.incomplete")
            )
        {
            return None;
        }
        if value["choices"][0]["delta"].get("tool_calls").is_some()
            || value["item"]["type"] == "function_call"
            || value["content_block"]["type"] == "tool_use"
            || value["candidates"][0]["content"]["parts"]
                .as_array()
                .is_some_and(|p| p.iter().any(|p| p.get("functionCall").is_some()))
        {
            return None;
        }
        match family {
            "openai_chat" => {
                if let Some(s) = value["choices"][0]["delta"]["content"].as_str() {
                    text.push_str(s);
                }
                if value["choices"][0]["finish_reason"] == "stop" {
                    stop = true;
                }
            }
            "openai_responses" => {
                if value["type"] == "response.output_text.delta" {
                    text.push_str(value["delta"].as_str()?);
                }
                if value["type"] == "response.completed" {
                    done = value["response"]["status"] == "completed";
                    stop = done;
                }
            }
            "anthropic_messages" => {
                if value["type"] == "content_block_delta" && value["delta"]["type"] == "text_delta"
                {
                    text.push_str(value["delta"]["text"].as_str()?);
                }
                if value["type"] == "message_delta" {
                    stop = value["delta"]["stop_reason"] == "end_turn";
                }
                if value["type"] == "message_stop" {
                    done = true;
                }
            }
            "gemini_generate_content" => {
                if let Some(parts) = value["candidates"][0]["content"]["parts"].as_array() {
                    for part in parts {
                        if part["thought"] != true {
                            if let Some(s) = part["text"].as_str() {
                                text.push_str(s);
                            }
                        }
                    }
                }
                if value["candidates"][0]["finishReason"] == "STOP" {
                    done = true;
                    stop = true;
                }
            }
            _ => return None,
        }
    }
    (done && stop && !text.trim().is_empty() && text.chars().count() <= 8192).then_some(text)
}
fn failed(status: Option<u16>, message: &str) -> ModelProbeResult {
    ModelProbeResult {
        status,
        usage: Value::Null,
        reply: Err(message.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::parse;
    #[test]
    fn dim_probe_requires_real_nonempty_completion_and_rejects_errors_tools_and_truncation() {
        for (family, event, terminal) in [
            ("openai_chat", r#"{"choices":[{"delta":{"content":"OK"},"finish_reason":"stop"}]}"#, "data: [DONE]\n\n"),
            ("openai_responses", r#"{"type":"response.output_text.delta","delta":"OK"}"#, "data: {\"type\":\"response.completed\",\"response\":{\"status\":\"completed\"}}\n\n"),
            ("anthropic_messages", r#"{"type":"content_block_delta","delta":{"type":"text_delta","text":"OK"}}"#, "data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"}}\n\ndata: {\"type\":\"message_stop\"}\n\n"),
            ("gemini_generate_content", r#"{"candidates":[{"content":{"parts":[{"text":"OK"}]}}]}"#, "data: {\"candidates\":[{\"finishReason\":\"STOP\"}]}\n\n"),
        ] {
            let partial = format!("data: {event}\n\n");
            assert!(parse(&partial, family).is_none(), "{family}");
            assert_eq!(parse(&format!("{partial}{terminal}"), family).as_deref(), Some("OK"), "{family}");
            assert!(parse(&format!("{partial}{terminal}data: {{\"error\":{{\"message\":\"failed\"}}}}\n\n"), family).is_none());
            assert!(parse(terminal, family).is_none());
            let tool = match family {
                "openai_chat" => r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"name":"weather"}}]}}]}"#,
                "openai_responses" => r#"{"type":"response.output_item.added","item":{"type":"function_call","name":"weather"}}"#,
                "anthropic_messages" => r#"{"type":"content_block_start","content_block":{"type":"tool_use","name":"weather"}}"#,
                _ => r#"{"candidates":[{"content":{"parts":[{"functionCall":{"name":"weather","args":{}}}]}}]}"#,
            };
            assert!(parse(&format!("{partial}data: {tool}\n\n{terminal}"), family).is_none());
        }
    }
}
