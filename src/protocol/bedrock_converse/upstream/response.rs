//! Bounded buffered accumulation and native Converse JSON decoding share Bedrock semantics.
use bytes::Bytes;
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};

use super::{
    frame::Decoder,
    invalid,
    state::{self, State, Update},
};
use crate::{
    error::GatewayError,
    protocol::{
        canonical::{CanonicalRelayResponse, CanonicalToolCall},
        stream_error::ProtocolStreamError,
        upstream_body::collect_bounded_upstream_body_with_provider,
    },
};

const PROVIDER: &str = "bedrock_converse_compatible";

pub async fn accumulate(
    response: rquest::Response,
    model: &str,
) -> Result<CanonicalRelayResponse, GatewayError> {
    let bytes =
        collect_bounded_upstream_body_with_provider(response, "Bedrock EventStream", PROVIDER)
            .await?;
    decode_eventstream(bytes, model).map_err(gateway)
}

pub(crate) fn decode_eventstream(
    bytes: Vec<u8>,
    model: &str,
) -> Result<CanonicalRelayResponse, ProtocolStreamError> {
    let mut decoder = Decoder::default();
    decoder.push(Bytes::from(bytes));
    let mut state = State::default();
    let mut snapshot = super::turn::Snapshot::default();
    let mut text = String::new();
    let mut tools: BTreeMap<u32, CanonicalToolCall> = BTreeMap::new();
    while let Some(event) = decoder.next()? {
        let update = state.observe(&event)?;
        if let Some(update) = &update {
            snapshot.accept(&event, update)?;
        }
        match update {
            Some(Update::Text(delta)) => text.push_str(&delta),
            Some(Update::ToolStart { index, id, name }) => {
                tools.insert(index, tool(id, name, String::new()));
            }
            Some(Update::ToolDelta { index, input }) => {
                tools
                    .get_mut(&index)
                    .and_then(|tool| tool.arguments.as_mut())
                    .ok_or_else(|| invalid("missing buffered tool identity"))?
                    .push_str(&input);
            }
            Some(Update::Start | Update::Reasoning(_)) | None => {}
        }
    }
    decoder.finish()?;
    let (reason, usage) = state.finish()?;
    for call in tools.values_mut() {
        if call
            .arguments
            .as_deref()
            .is_none_or(|args| args.trim().is_empty())
        {
            call.arguments = Some("{}".into());
        }
    }
    let bedrock_turn = snapshot.finish()?;
    let (text, tool_calls) = bedrock_turn
        .as_ref()
        .map(|turn| turn.projection())
        .unwrap_or_else(|| (text, tools.into_values().collect()));
    Ok(CanonicalRelayResponse {
        bedrock_turn,
        sources: Vec::new(),
        vertex_turn: None,
        model_declaration: None,
        model: model.into(),
        text,
        usage: Some(usage),
        tool_calls,
        upstream_status: Some(200),
        finish_reason: Some(reason.into()),
    })
}

pub fn unpack(body: &Value) -> Result<CanonicalRelayResponse, GatewayError> {
    unpack_native(body).map_err(gateway)
}

fn unpack_native(body: &Value) -> Result<CanonicalRelayResponse, ProtocolStreamError> {
    let message = body
        .pointer("/output/message")
        .ok_or_else(|| invalid("missing Converse message"))?;
    if message.get("role").and_then(Value::as_str) != Some("assistant") {
        return Err(invalid("invalid Converse message role"));
    }
    let content = message
        .get("content")
        .and_then(Value::as_array)
        .filter(|c| c.len() <= 4096)
        .ok_or_else(|| invalid("invalid Converse content"))?;
    let bedrock_turn = crate::protocol::bedrock_turn::wire::capture(content)
        .map_err(|_| invalid("invalid or oversized Converse reasoning turn"))?;
    let mut text = String::new();
    let mut tool_calls = Vec::new();
    for block in content {
        if block.as_object().is_none_or(|map| map.len() != 1) {
            return Err(invalid("invalid Converse block"));
        }
        if let Some(value) = block.get("text").and_then(Value::as_str) {
            text.push_str(value);
        } else if let Some(value) = block.get("toolUse") {
            let identity = |key| {
                value
                    .get(key)
                    .and_then(Value::as_str)
                    .filter(|s| !s.trim().is_empty())
                    .map(str::to_owned)
                    .ok_or_else(|| invalid("missing Converse tool identity"))
            };
            let input = value
                .get("input")
                .ok_or_else(|| invalid("missing Converse tool input"))?;
            tool_calls.push(tool(
                identity("toolUseId")?,
                identity("name")?,
                input.to_string(),
            ));
        } else if block.get("reasoningContent").is_some() {
            // Validated and retained in the source-scoped turn, never exposed as answer text.
        } else {
            return Err(invalid("unsupported Converse content block"));
        }
    }
    let finish_reason = state::stop_reason(body.get("stopReason").and_then(Value::as_str))?;
    if finish_reason == "tool_calls" && tool_calls.is_empty() {
        return Err(invalid("tool stop reason has no tool identity"));
    }
    Ok(CanonicalRelayResponse {
        bedrock_turn,
        sources: Vec::new(),
        vertex_turn: None,
        model_declaration: None,
        model: body
            .get("model")
            .or_else(|| body.get("modelVersion"))
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .into(),
        text,
        tool_calls,
        usage: body.get("usage").map(state::usage).transpose()?,
        upstream_status: Some(200),
        finish_reason: Some(finish_reason.into()),
    })
}

fn tool(id: String, name: String, arguments: String) -> CanonicalToolCall {
    CanonicalToolCall {
        id: Some(id),
        name: Some(name),
        arguments: Some(arguments),
        call_type: "function".into(),
        raw: HashMap::new(),
    }
}

fn gateway(error: ProtocolStreamError) -> GatewayError {
    if let Some(classified) = error.gateway_error() {
        return classified.clone();
    }
    let code = if error.is_upstream_failure() {
        "bedrock_stream_upstream_error"
    } else {
        "bedrock_stream_invalid"
    };
    GatewayError::server_error(error.to_string())
        .with_code(code)
        .with_provider(PROVIDER)
}
