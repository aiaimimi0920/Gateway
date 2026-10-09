//! Bounded conventional endpoints; protocol identity comes from generated output, not HTTP 200.
pub use super::endpoint_candidates::bases;
use super::DiscoveredProtocol as P;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub const ALL: [P; 12] = [
    P::ChatCompletions,
    P::Responses,
    P::Messages,
    P::GeminiGenerateContent,
    P::GeminiInteractions,
    P::OllamaChat,
    P::OllamaGenerate,
    P::CohereChat,
    P::BedrockConverse,
    P::Completions,
    P::DashscopeText,
    P::DashscopeMultimodal,
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProbeAttempt {
    pub endpoint: String,
    pub model: Option<String>,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http_status: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolProbe {
    pub protocol: P,
    #[serde(default)]
    pub routing_ready: bool,
    /// A timeout is inconclusive, not evidence that the service lacks this protocol.
    pub status: String,
    pub attempts: Vec<ProbeAttempt>,
}

pub fn request(protocol: P, model: &str) -> (String, Value) {
    let prompt = "Reply with only OK.";
    let encoded: String = url::form_urlencoded::byte_serialize(model.as_bytes())
        .collect::<String>()
        .replace('+', "%20");
    match protocol {
        P::DashscopeText | P::DashscopeMultimodal => (
            if protocol == P::DashscopeText {
                crate::protocol::dashscope::TEXT_PATH
            } else {
                crate::protocol::dashscope::MULTIMODAL_PATH
            }
            .into(),
            json!({"model":model,"input":{"messages":[{"role":"user","content":if protocol == P::DashscopeText { json!(prompt) } else { json!([{"text":prompt}]) }}]},"parameters":{"max_tokens":256,"result_format":"message"}}),
        ),
        P::ChatCompletions => (
            "/chat/completions".into(),
            json!({"model":model,"messages":[{"role":"user","content":prompt}],"max_tokens":256,"stream":false}),
        ),
        P::Responses => (
            "/responses".into(),
            json!({"model":model,"input":prompt,"max_output_tokens":256,"stream":false,"store":false}),
        ),
        P::Messages => (
            "/messages".into(),
            json!({"model":model,"messages":[{"role":"user","content":prompt}],"max_tokens":256,"stream":false}),
        ),
        P::GeminiGenerateContent => (
            format!("/models/{encoded}:generateContent"),
            json!({"contents":[{"role":"user","parts":[{"text":prompt}]}],"generationConfig":{"maxOutputTokens":256}}),
        ),
        P::GeminiInteractions => (
            "/interactions".into(),
            json!({"model":model,"input":prompt,"store":false,"generation_config":{"max_output_tokens":256}}),
        ),
        P::OllamaChat => (
            "/chat".into(),
            json!({"model":model,"messages":[{"role":"user","content":prompt}],"options":{"num_predict":256},"stream":false}),
        ),
        P::OllamaGenerate => (
            "/generate".into(),
            json!({"model":model,"prompt":prompt,"options":{"num_predict":256},"stream":false}),
        ),
        P::CohereChat => (
            "/chat".into(),
            json!({"model":model,"messages":[{"role":"user","content":prompt}],"max_tokens":256,"stream":false}),
        ),
        P::BedrockConverse => (
            format!("/model/{encoded}/converse"),
            json!({"messages":[{"role":"user","content":[{"text":prompt}]}],"inferenceConfig":{"maxTokens":256}}),
        ),
        P::Completions => (
            "/completions".into(),
            json!({"model":model,"prompt":prompt,"max_tokens":256,"stream":false}),
        ),
    }
}

pub fn authenticate(
    request: rquest::RequestBuilder,
    protocol: P,
    key: &str,
) -> rquest::RequestBuilder {
    match protocol {
        P::Messages => request
            .header("x-api-key", key)
            .header("anthropic-version", "2023-06-01"),
        P::GeminiGenerateContent => request.header("x-goog-api-key", key),
        P::GeminiInteractions => request
            .header("x-goog-api-key", key)
            .header("Api-Revision", "2026-05-20"),
        // Bedrock API-key bearer mode only. AWS signing requires explicit account metadata.
        _ => request.bearer_auth(key),
    }
}

pub fn has_reply(protocol: P, body: &Value) -> bool {
    if body.get("error").is_some_and(|v| !v.is_null()) {
        return false;
    }
    let text = |v: &Value| v.as_str().is_some_and(|v| !v.trim().is_empty());
    let parts = |v: &Value| {
        v.as_array()
            .is_some_and(|a| a.iter().any(|v| v.get("text").is_some_and(text)))
    };
    match protocol {
        P::DashscopeText | P::DashscopeMultimodal => {
            !body
                .get("code")
                .and_then(Value::as_str)
                .is_some_and(|v| !v.is_empty())
                && (body.pointer("/output/text").is_some_and(text)
                    || body
                        .pointer("/output/choices/0/message/content")
                        .is_some_and(|v| text(v) || parts(v)))
        }
        P::ChatCompletions => body.pointer("/choices/0/message/content").is_some_and(text),
        P::Responses => body
            .get("output")
            .and_then(Value::as_array)
            .is_some_and(|items| {
                items
                    .iter()
                    .any(|item| item.get("content").is_some_and(parts))
            }),
        P::Messages => body.get("content").is_some_and(parts),
        P::GeminiGenerateContent => body
            .pointer("/candidates/0/content/parts")
            .is_some_and(parts),
        P::GeminiInteractions => {
            body.get("outputs").is_some_and(parts)
                || body
                    .get("steps")
                    .and_then(Value::as_array)
                    .is_some_and(|steps| steps.iter().any(|s| s.get("content").is_some_and(parts)))
        }
        P::OllamaChat => {
            body.get("done").and_then(Value::as_bool) == Some(true)
                && body.pointer("/message/content").is_some_and(text)
        }
        P::OllamaGenerate => {
            body.get("done").and_then(Value::as_bool) == Some(true)
                && body.get("response").is_some_and(text)
        }
        P::CohereChat => body.pointer("/message/content").is_some_and(parts),
        P::BedrockConverse => body.pointer("/output/message/content").is_some_and(parts),
        P::Completions => body.pointer("/choices/0/text").is_some_and(text),
    }
}
