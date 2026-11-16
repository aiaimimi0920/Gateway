//! Ordered Google Parts are explicit Gateway state, never another vendor's native reasoning.
use crate::error::{FallbackHint, GatewayError};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
mod stream;
#[cfg(test)]
mod stream_tests;
#[cfg(test)]
mod tests;
pub(crate) mod wire;

pub(crate) const WIRE_FIELD: &str = "gateway_vertex_turn";
pub(crate) const HISTORY_FIELD: &str = "gateway_vertex_history";
pub(crate) const INVALID: &str = "Invalid Gateway Vertex assistant-turn extension.";
pub(crate) const MAX_TURN_BYTES: usize = 4 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VertexTurnSource {
    #[serde(rename = "vertex_express_api")]
    Express,
    #[serde(rename = "vertex_user_oauth")]
    UserOAuth,
    #[serde(rename = "dim_user_oauth")]
    DimUserOAuth,
}

/// Requested upstream model, not a caller alias or an observed modelVersion.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VertexAssistantTurn {
    pub source: VertexTurnSource,
    pub model: String,
    pub parts: Vec<VertexTurnPart>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VertexFunctionCall {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub args: Option<Map<String, Value>>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ThoughtSignature(pub String);
impl std::fmt::Debug for ThoughtSignature {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("[opaque Google thought signature]")
    }
}

/// A validated oneof: text OR complete function call. Empty text remains a real Part.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VertexTurnPart {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub function_call: Option<VertexFunctionCall>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub public_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thought: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thought_signature: Option<ThoughtSignature>,
}

impl VertexAssistantTurn {
    pub(crate) fn visible_text(&self) -> String {
        self.parts
            .iter()
            .filter(|p| p.thought != Some(true))
            .filter_map(|p| p.text.as_deref())
            .collect()
    }
    pub(crate) fn google_parts(&self) -> Value {
        json!(self
            .parts
            .iter()
            .map(|p| {
                let mut value = if let Some(text) = &p.text {
                    json!({"text":text})
                } else {
                    let call = p.function_call.as_ref().expect("validated Part oneof");
                    let mut value = json!({"functionCall":{"name":call.name}});
                    if let Some(args) = &call.args {
                        value["functionCall"]["args"] = json!(args);
                    }
                    value
                };
                if let Some(thought) = p.thought {
                    value["thought"] = json!(thought);
                }
                if let Some(signature) = &p.thought_signature {
                    value["thoughtSignature"] = json!(signature);
                }
                value
            })
            .collect::<Vec<_>>())
    }
}

pub(crate) fn invalid() -> GatewayError {
    let mut error = GatewayError::server_error(INVALID).with_code("invalid_vertex_turn");
    error.retryable = false;
    error.fallback_hint = FallbackHint::Abort {
        reason: INVALID.into(),
    };
    error
}

pub(crate) fn read(body: &Value) -> Result<Option<VertexAssistantTurn>, GatewayError> {
    body.get(WIRE_FIELD).map(wire::read_turn).transpose()
}

pub(crate) fn read_response(
    body: &Value,
    text: &str,
    calls: &[super::canonical::CanonicalToolCall],
) -> Result<Option<VertexAssistantTurn>, GatewayError> {
    let turn = read(body)?;
    if let Some(turn) = &turn {
        if turn.visible_text() != text {
            return Err(invalid());
        }
        let parts: Vec<_> = turn
            .parts
            .iter()
            .filter(|p| p.function_call.is_some())
            .collect();
        if parts.len() != calls.len() {
            return Err(invalid());
        }
        for (part, call) in parts.into_iter().zip(calls) {
            let function = part.function_call.as_ref().expect("filtered function");
            let args = call
                .arguments
                .as_deref()
                .map(serde_json::from_str::<Value>)
                .transpose()
                .map_err(|_| invalid())?
                .unwrap_or_else(|| json!({}));
            if part.public_id != call.id
                || call.name.as_deref() != Some(function.name.as_str())
                || args
                    != function
                        .args
                        .as_ref()
                        .map(|a| json!(a))
                        .unwrap_or_else(|| json!({}))
            {
                return Err(invalid());
            }
        }
    }
    Ok(turn)
}

pub(crate) fn project(body: &mut Value, turn: Option<&VertexAssistantTurn>) {
    if let Some(turn) = turn {
        body[WIRE_FIELD] = json!(turn);
    }
}
