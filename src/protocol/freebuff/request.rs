use super::{normalize_lookup_key, FREEBUFF_DEFAULT_CHAT_COMPLETIONS_PATH};
use crate::error::GatewayError;
use crate::protocol::canonical::{CanonicalRelayRequest, EndpointKind};
use crate::protocol::openai;
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::common::RequestPlan;
use rquest::Method;
use serde_json::{Map, Value};

pub fn normalize_base_url(base_url: &str) -> String {
    let trimmed = base_url.trim().trim_end_matches('/');
    for (prefix, canonical) in [
        ("https://codebuff.com", "https://www.codebuff.com"),
        ("http://codebuff.com", "http://www.codebuff.com"),
    ] {
        if let Some(rest) = trimmed.strip_prefix(prefix) {
            // Check the complete authority suffix without reserializing the URL.
            let authority_suffix = rest.split(['/', '?', '#']).next().unwrap_or_default();
            let is_port = authority_suffix.strip_prefix(':').is_some_and(|port| {
                !port.is_empty() && port.bytes().all(|byte| byte.is_ascii_digit())
            });
            if authority_suffix.is_empty() || is_port {
                return format!("{canonical}{rest}");
            }
        }
    }
    trimmed.to_string()
}

pub fn is_reserved_payload_extra_key(key: &str) -> bool {
    matches!(
        normalize_lookup_key(key).as_str(),
        "freebuffagentid"
            | "freebuffbusiness"
            | "freebuffmodelagentmap"
            | "freebuffrunrotationsecs"
            | "freebuffagentrunspath"
            | "freebuffstartrunpath"
            | "freebufffinishrunpath"
            | "freebuffsessionpath"
            | "freebuffsessionpollintervalms"
            | "freebuffsessionpolltimeoutms"
            | "freebuffcostmode"
            | "freebuffuseragent"
    )
}

pub fn build_request_plan(
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
    stream: bool,
) -> Result<RequestPlan, GatewayError> {
    super::business_target::reject_legacy_execution(payload)?;
    match req.endpoint_kind {
        EndpointKind::ChatCompletions
        | EndpointKind::Messages
        | EndpointKind::Responses
        | EndpointKind::Completions => {
            let path = payload
                .chat_completions_path
                .as_deref()
                .unwrap_or(FREEBUFF_DEFAULT_CHAT_COMPLETIONS_PATH);
            Ok(RequestPlan {
                method: Method::POST,
                url: format!("{}{}", normalize_base_url(&payload.base_url), path),
                query: Vec::new(),
                body: Some(pack_freebuff_body(req, model, stream)),
                response_kind: EndpointKind::ChatCompletions,
            })
        }
        _ => Err(GatewayError::bad_request(
            "FreeBuff adapters currently support only chat/messages/responses style endpoints",
        )
        .with_code("unsupported_freebuff_endpoint")),
    }
}

pub(super) fn build_chat_request_body(
    req: &CanonicalRelayRequest,
    model: &str,
    stream: bool,
    run_id: &str,
    cost_mode: &str,
    freebuff_instance_id: Option<&str>,
) -> Value {
    let mut body = pack_freebuff_body(req, model, stream);
    sanitize_freebuff_messages(&mut body);
    apply_runtime_metadata(
        &mut body,
        run_id,
        cost_mode,
        &generate_client_session_id(),
        freebuff_instance_id,
    );
    body
}

/// Runtime identity always wins over caller extensions. Catalog execution also uses this on its
/// final harness body before freezing/signing; it never synthesizes the harness's system prompt.
pub(super) fn apply_runtime_metadata(
    body: &mut Value,
    run_id: &str,
    cost_mode: &str,
    client_id: &str,
    freebuff_instance_id: Option<&str>,
) {
    let object = body
        .as_object_mut()
        .expect("OpenAI-compatible payload must serialize as an object");
    let metadata = object
        .entry("codebuff_metadata".to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    if !metadata.is_object() {
        *metadata = Value::Object(Map::new());
    }
    if let Some(metadata) = metadata.as_object_mut() {
        metadata.retain(|key, _| {
            !key.starts_with("freebuff_")
                && !matches!(
                    key.as_str(),
                    "run_id" | "cost_mode" | "client_id" | "surface"
                )
        });
        metadata.insert("run_id".to_string(), Value::String(run_id.to_string()));
        metadata.insert(
            "cost_mode".to_string(),
            Value::String(cost_mode.trim().to_string()),
        );
        metadata.insert(
            "client_id".to_string(),
            Value::String(client_id.to_string()),
        );
        if let Some(instance_id) = freebuff_instance_id
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            metadata.insert(
                "freebuff_instance_id".to_string(),
                Value::String(instance_id.to_string()),
            );
            if instance_id
                .strip_prefix("cli:")
                .is_some_and(|id| !id.is_empty())
            {
                metadata.insert("freebuff_multi_session".into(), Value::String("1".into()));
                metadata.insert("surface".into(), Value::String("cli".into()));
            }
        }
    }
}

fn pack_freebuff_body(req: &CanonicalRelayRequest, model: &str, stream: bool) -> Value {
    let mut body = openai::pack_openai(req, model, stream);
    let object = body
        .as_object_mut()
        .expect("OpenAI-compatible payload must serialize as an object");
    // Provider runtime options are not upstream request fields, including when supplied by a caller.
    object.retain(|key, _| !is_reserved_payload_extra_key(key));
    if let Some(metadata) = object.get_mut("codebuff_metadata") {
        // Malformed extension data must not suppress the executor's required run/session envelope.
        if !metadata.is_object() {
            *metadata = Value::Object(Map::new());
        }
        metadata
            .as_object_mut()
            .expect("metadata was normalized to an object")
            .retain(|key, _| {
                !key.starts_with("freebuff_")
                    && !matches!(
                        key.as_str(),
                        "run_id" | "cost_mode" | "client_id" | "surface"
                    )
            });
    }
    body
}

pub(super) fn sanitize_freebuff_messages(body: &mut Value) {
    let Some(messages) = body.get_mut("messages").and_then(Value::as_array_mut) else {
        return;
    };

    for message in messages {
        let Some(content) = message.get_mut("content") else {
            continue;
        };

        let Some(parts) = content.as_array() else {
            continue;
        };

        if let Some(flattened) = flatten_freebuff_content_parts(parts) {
            *content = Value::String(flattened);
        }
    }
}

pub(super) fn flatten_freebuff_content_parts(parts: &[Value]) -> Option<String> {
    let mut segments = Vec::with_capacity(parts.len());
    for part in parts {
        let object = part.as_object()?;
        match object.get("type").and_then(Value::as_str) {
            Some("text") => {
                let text = object.get("text").and_then(Value::as_str)?;
                segments.push(text.to_string());
            }
            Some("json") => {
                let value = object.get("value")?;
                segments.push(value.to_string());
            }
            Some("input_text") => {
                let text = object
                    .get("text")
                    .or_else(|| object.get("value"))
                    .and_then(Value::as_str)?;
                segments.push(text.to_string());
            }
            Some("output_text") => {
                let text = object.get("text").and_then(Value::as_str)?;
                segments.push(text.to_string());
            }
            Some("image_url") | Some("input_image") => return None,
            _ => {
                if let Some(text) = object.get("text").and_then(Value::as_str) {
                    segments.push(text.to_string());
                } else if let Some(value) = object.get("value") {
                    segments.push(value.to_string());
                } else {
                    return None;
                }
            }
        }
    }

    Some(segments.join("\n"))
}

pub(super) fn generate_client_session_id() -> String {
    const ALPHABET: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let mut rng = rand::thread_rng();
    (0..13)
        .map(|_| {
            let index = rand::Rng::gen_range(&mut rng, 0..ALPHABET.len());
            ALPHABET[index] as char
        })
        .collect()
}
