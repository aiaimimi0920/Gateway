//! Credential-scoped Codex generation proof; HTTP success alone is insufficient.
use std::time::{Duration, Instant};

use futures::StreamExt;
use serde_json::{json, Value};

use super::console_model_probe::ModelProbeResult;
use super::{ProviderPayloadProbeReport, ProviderPayloadProbeStatus};
use crate::{routing::config::CredentialProbeTarget, state::AppState};

pub(super) async fn probe(
    state: &AppState,
    target: &CredentialProbeTarget,
) -> (String, ProviderPayloadProbeReport) {
    let model = target.payload.default_model.as_deref().unwrap_or("gpt-5.4");
    let url = format!(
        "{}/responses",
        target.payload.base_url.trim_end_matches('/')
    );
    let point = format!(
        "POST {} (model: {model})",
        crate::console::secrets::redact_url_value(&url)
    );
    let audit = super::model_probe_recording::begin(state, target, model, true).await;
    let started = Instant::now();
    let result = tokio::time::timeout(
        Duration::from_secs(60),
        generate(state, target, &url, model),
    )
    .await
    .unwrap_or_else(|_| failed(None, "Model call timed out after 60 seconds."));
    super::model_probe_recording::finish(
        state,
        target,
        model,
        audit.as_deref(),
        started.elapsed(),
        &result,
    )
    .await;
    let (status, message) = match result.reply {
        Ok(reply) => (
            ProviderPayloadProbeStatus::Passed,
            format!("Model call passed. Model: {model}. Reply: {reply}"),
        ),
        Err(error) => (
            ProviderPayloadProbeStatus::Failed,
            format!("Model call failed. Model: {model}. {error}"),
        ),
    };
    (point, ProviderPayloadProbeReport { status, message })
}

async fn generate(
    state: &AppState,
    target: &CredentialProbeTarget,
    url: &str,
    model: &str,
) -> ModelProbeResult {
    let body = json!({"model": model, "stream": true, "store": false,
        "instructions": "Reply briefly.",
        "input": [{"role": "user", "content": [{"type": "input_text", "text": "Reply with only OK."}]}]});
    let request = state
        .upstream_client
        .client()
        .post(url)
        .headers(crate::upstream::headers::build_upstream_headers(
            &target.payload,
        ))
        .header("Accept", "text/event-stream")
        .json(&body);
    let response = match request.send().await {
        Ok(response) => response,
        Err(_) => return failed(None, "Cannot reach the ChatGPT model endpoint."),
    };
    let status = response.status().as_u16();
    if !response.status().is_success() {
        return failed(
            Some(status),
            &format!("HTTP {status}: upstream rejected the model request."),
        );
    }
    let stream = response.bytes_stream();
    futures::pin_mut!(stream);
    let mut bytes = Vec::new();
    loop {
        match stream.next().await {
            Some(Ok(chunk)) if bytes.len() + chunk.len() <= 262_144 => {
                bytes.extend_from_slice(&chunk)
            }
            Some(Ok(_)) => return failed(Some(status), "Model response exceeded 256 KiB."),
            None => break,
            Some(Err(_)) => return failed(Some(status), "Model response was interrupted."),
        }
    }
    let Some((mut reply, usage)) = completed_reply(&bytes) else {
        return failed(
            Some(status),
            "No completed non-empty assistant reply was returned.",
        );
    };
    for secret in std::iter::once(target.payload.api_key.as_str())
        .chain(target.payload.auth_token.as_deref())
        .chain(target.payload.headers.values().map(String::as_str))
        .filter(|value| !value.is_empty())
    {
        reply = reply.replace(secret, "[redacted]");
    }
    ModelProbeResult {
        status: Some(status),
        usage,
        reply: Ok(crate::error::sanitize_provider_error_message(&reply)
            .chars()
            .take(320)
            .collect()),
    }
}

fn completed_reply(bytes: &[u8]) -> Option<(String, Value)> {
    let text = std::str::from_utf8(bytes).ok()?;
    let mut completed_items = std::collections::BTreeMap::new();
    for line in text.lines() {
        let Some(data) = line.strip_prefix("data:") else {
            continue;
        };
        let Ok(event) = serde_json::from_str::<Value>(data.trim()) else {
            continue;
        };
        if matches!(
            event["type"].as_str(),
            Some("error" | "response.failed" | "response.incomplete")
        ) {
            return None;
        }
        if event["type"] == "response.output_item.done" && event["item"]["status"] == "completed" {
            if let Some(index) = event["output_index"].as_u64() {
                completed_items.insert(index, event["item"].clone());
            }
        }
        if event["type"] != "response.completed" {
            continue;
        }
        let response = &event["response"];
        if response["status"] != "completed" || response.get("error").is_some_and(|v| !v.is_null())
        {
            return None;
        }
        let output = response["output"].as_array()?;
        // Codex may finalize messages in item.done and leave terminal output empty.
        // Only finalized items plus a successful response.completed prove success.
        let items: Vec<&Value> = if output.is_empty() {
            completed_items.values().collect()
        } else {
            output.iter().collect()
        };
        let mut reply = String::new();
        for item in items {
            if item["type"] != "message" || item["role"] != "assistant" {
                continue;
            }
            for part in item["content"].as_array()? {
                if part["type"] == "output_text" {
                    reply.push_str(part["text"].as_str().unwrap_or_default());
                }
            }
        }
        return (!reply.trim().is_empty())
            .then(|| (reply.trim().to_string(), response["usage"].clone()));
    }
    None
}

fn failed(status: Option<u16>, message: &str) -> ModelProbeResult {
    ModelProbeResult {
        status,
        reply: Err(message.into()),
        usage: Value::Null,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requires_completed_assistant_output() {
        assert!(completed_reply(
            b"data: {\"type\":\"response.output_text.delta\",\"delta\":\"OK\"}\n"
        )
        .is_none());
        let event = json!({"type":"response.completed", "response": {"status":"completed", "output":[
            {"type":"message","role":"assistant","content":[{"type":"output_text","text":"OK"}]}], "usage":{"output_tokens":1}}});
        let stream = format!("data: {event}\r\n\r\ndata: [DONE]\r\n");
        let (reply, usage) = completed_reply(stream.as_bytes()).expect("completed reply");
        assert_eq!(reply, "OK");
        assert_eq!(usage["output_tokens"], 1);
        assert!(completed_reply(b"data: {\"type\":\"response.failed\"}\n").is_none());
    }

    #[test]
    fn accepts_completed_items_when_terminal_output_is_empty() {
        let item = json!({"type":"response.output_item.done", "output_index":0,
            "item":{"type":"message", "role":"assistant", "status":"completed",
            "content":[{"type":"output_text", "text":"OK"}]}});
        let completed = json!({"type":"response.completed", "response":{
            "status":"completed", "output":[], "usage":{"output_tokens":1}}});
        let stream = format!("data: {item}\n\ndata: {completed}\n\ndata: [DONE]\n\n");
        let (reply, usage) = completed_reply(stream.as_bytes()).expect("completed streamed item");
        assert_eq!(reply, "OK");
        assert_eq!(usage["output_tokens"], 1);

        // A finalized message still cannot turn a failed or truncated response into success.
        assert!(completed_reply(format!("data: {item}\n").as_bytes()).is_none());
        for failure in ["error", "response.failed", "response.incomplete"] {
            let stream =
                format!("data: {item}\ndata: {{\"type\":\"{failure}\"}}\ndata: {completed}\n");
            assert!(completed_reply(stream.as_bytes()).is_none());
        }
    }

    #[test]
    fn terminal_output_wins_and_streamed_items_are_ordered_and_deduplicated() {
        let item = |index, text| {
            json!({"type":"response.output_item.done", "output_index":index,
            "item":{"type":"message", "role":"assistant", "status":"completed",
            "content":[{"type":"output_text", "text":text}]}})
        };
        let first = item(0, "O");
        let second = item(1, "K");
        let mut completed = json!({"type":"response.completed", "response":{
            "status":"completed", "output":[]}});
        let stream = format!("data: {second}\ndata: {first}\ndata: {first}\ndata: {completed}\n");
        assert_eq!(completed_reply(stream.as_bytes()).unwrap().0, "OK");
        completed["response"]["output"] = json!([item(0, "canonical")["item"]]);
        let stream = format!("data: {first}\ndata: {completed}\n");
        assert_eq!(completed_reply(stream.as_bytes()).unwrap().0, "canonical");

        completed["response"]["output"] = json!([]);
        for (field, value) in [
            ("role", "user"),
            ("type", "reasoning"),
            ("status", "in_progress"),
        ] {
            let mut invalid = item(0, "not an assistant reply");
            invalid["item"][field] = json!(value);
            let stream = format!("data: {invalid}\ndata: {completed}\n");
            assert!(completed_reply(stream.as_bytes()).is_none());
        }
    }
}
