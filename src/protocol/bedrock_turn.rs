//! Ordered Bedrock response state; not Anthropic/OpenAI reasoning or authority to replay history.
use super::canonical::CanonicalToolCall;
use crate::error::{FallbackHint, GatewayError};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
pub(crate) mod continuation;
pub(crate) mod wire;

pub(crate) const WIRE_FIELD: &str = "gateway_bedrock_turn";
pub(crate) const HISTORY_FIELD: &str = "gateway_bedrock_history";
pub(crate) const MAX_TURN_BYTES: usize = 4 * 1024 * 1024;
const INVALID: &str = "Invalid Gateway Bedrock assistant-turn extension.";

#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BedrockAssistantTurn {
    pub(crate) content: Vec<BedrockTurnBlock>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) continuation: Option<continuation::Continuation>,
}

// Opaque signatures and reasoning must never enter derived canonical Debug logs.
impl std::fmt::Debug for BedrockAssistantTurn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("[opaque Bedrock assistant turn]")
    }
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub(crate) enum BedrockTurnBlock {
    #[serde(rename = "text")]
    Text(String),
    #[serde(rename = "toolUse")]
    Tool(BedrockToolUse),
    #[serde(rename = "reasoningContent")]
    Reasoning(BedrockReasoning),
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct BedrockToolUse {
    pub tool_use_id: String,
    pub name: String,
    pub input: Value,
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub(crate) enum BedrockReasoning {
    #[serde(rename = "reasoningText")]
    Text(BedrockReasoningText),
    #[serde(rename = "redactedContent")]
    Redacted(String),
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BedrockReasoningText {
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
}

impl BedrockAssistantTurn {
    pub(crate) fn native_content(&self) -> Value {
        json!(self.content)
    }
    pub(crate) fn projection(&self) -> (String, Vec<CanonicalToolCall>) {
        let mut text = String::new();
        let mut tools = Vec::new();
        for block in &self.content {
            match block {
                BedrockTurnBlock::Text(value) => text.push_str(value),
                BedrockTurnBlock::Tool(call) => tools.push(CanonicalToolCall {
                    id: Some(call.tool_use_id.clone()),
                    name: Some(call.name.clone()),
                    arguments: Some(call.input.to_string()),
                    call_type: "function".into(),
                    raw: Default::default(),
                }),
                BedrockTurnBlock::Reasoning(_) => {}
            }
        }
        (text, tools)
    }
}

pub(crate) fn invalid() -> GatewayError {
    let mut error = GatewayError::server_error(INVALID).with_code("invalid_bedrock_turn");
    error.retryable = false;
    error.fallback_hint = FallbackHint::Abort {
        reason: INVALID.into(),
    };
    error
}

pub(crate) fn read_response(
    body: &Value,
    text: &str,
    calls: &[CanonicalToolCall],
) -> Result<Option<BedrockAssistantTurn>, GatewayError> {
    if body.get(WIRE_FIELD).is_some() && body.get(super::vertex_turn::WIRE_FIELD).is_some() {
        return Err(invalid());
    }
    let turn = body.get(WIRE_FIELD).map(wire::read_turn).transpose()?;
    if let Some(turn) = &turn {
        let (visible, tools) = turn.projection();
        if visible != text || tools.len() != calls.len() {
            return Err(invalid());
        }
        for (expected, call) in tools.iter().zip(calls) {
            let args = call
                .arguments
                .as_deref()
                .map(serde_json::from_str::<Value>)
                .transpose()
                .map_err(|_| invalid())?;
            if expected.id != call.id
                || expected.name != call.name
                || !matches!(call.call_type.as_str(), "function" | "function_call")
                || args
                    != expected
                        .arguments
                        .as_deref()
                        .map(serde_json::from_str::<Value>)
                        .transpose()
                        .map_err(|_| invalid())?
            {
                return Err(invalid());
            }
        }
    }
    Ok(turn)
}

pub(crate) fn project(body: &mut Value, turn: Option<&BedrockAssistantTurn>) {
    if let Some(turn) = turn {
        body[WIRE_FIELD] = json!(turn);
    }
}

/// Run before every product/native shortcut, after resolving the effective call-flow payload.
pub(crate) fn validate_request(
    payload: &crate::routing::candidate::ProviderAccountPayload,
    req: &crate::protocol::canonical::CanonicalRelayRequest,
) -> Result<(), GatewayError> {
    if req
        .extra
        .keys()
        .any(|k| k == HISTORY_FIELD || response_field(k))
        || req
            .raw_body
            .as_object()
            .is_some_and(|v| v.keys().any(|k| response_field(k)))
        || payload
            .extra_body
            .as_ref()
            .is_some_and(|v| v.keys().any(|k| k == HISTORY_FIELD || response_field(k)))
    {
        return Err(GatewayError::bad_request(
            "Bedrock continuation requires an explicit history envelope.",
        )
        .with_code("bedrock_invalid_signed_history"));
    }
    if req.raw_body.get(HISTORY_FIELD).is_some()
        && payload.canonical_adapter() != "bedrock_converse_compatible"
    {
        return Err(GatewayError::bad_request(
            "Bedrock assistant state cannot target another product.",
        )
        .with_code("bedrock_signed_history_wrong_product"));
    }
    Ok(())
}

pub(crate) fn response_field(key: &str) -> bool {
    matches!(
        key,
        "gateway_bedrock_turn" | "gateway_bedrock_event" | "gateway_bedrock_terminal"
    )
}
