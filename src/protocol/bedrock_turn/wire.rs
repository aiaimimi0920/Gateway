//! Bound opaque response state before cloning and validate both native unions.
use super::*;
use base64::{engine::general_purpose::STANDARD, Engine};
use std::{collections::HashSet, io::Write};

struct Budget(usize);
impl Write for Budget {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0 = self
            .0
            .checked_sub(bytes.len())
            .ok_or_else(|| std::io::Error::other("Bedrock turn budget exceeded"))?;
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
pub(crate) fn within_budget(value: &impl Serialize) -> Result<(), GatewayError> {
    within_limit(value, MAX_TURN_BYTES)
}
pub(crate) fn within_limit(value: &impl Serialize, limit: usize) -> Result<(), GatewayError> {
    serde_json::to_writer(Budget(limit), value).map_err(|_| invalid())
}

pub(crate) fn capture(content: &[Value]) -> Result<Option<BedrockAssistantTurn>, GatewayError> {
    if !content.iter().any(|b| b.get("reasoningContent").is_some()) {
        return Ok(None);
    }
    within_budget(&content)?;
    read_turn(&json!({"content":content})).map(Some)
}

pub(crate) fn read_turn(value: &Value) -> Result<BedrockAssistantTurn, GatewayError> {
    // The optional new witness has its own bounded allowance, not a reduction of the old content budget.
    serde_json::to_writer(Budget(MAX_TURN_BYTES + 4096), value).map_err(|_| invalid())?;
    let content = value
        .get("content")
        .and_then(Value::as_array)
        .filter(|c| !c.is_empty() && c.len() <= 4096)
        .ok_or_else(invalid)?;
    let mut reasoning = false;
    for block in content {
        if block.as_object().is_none_or(|o| o.len() != 1) {
            return Err(invalid());
        }
        if let Some(value) = block.get("reasoningContent") {
            reasoning = true;
            if value.as_object().is_none_or(|o| o.len() != 1) {
                return Err(invalid());
            }
            if value
                .get("reasoningText")
                .and_then(|t| t.get("signature"))
                .is_some_and(|v| !v.is_string())
            {
                return Err(invalid());
            }
        }
    }
    let turn: BedrockAssistantTurn =
        serde_json::from_value(value.clone()).map_err(|_| invalid())?;
    if !reasoning {
        return Err(invalid());
    }
    validate(&turn)?;
    Ok(turn)
}

pub(crate) fn validate(turn: &BedrockAssistantTurn) -> Result<(), GatewayError> {
    #[derive(Serialize)]
    struct Content<'a> {
        content: &'a [BedrockTurnBlock],
    }
    within_budget(&Content {
        content: &turn.content,
    })?;
    if let Some(scope) = &turn.continuation {
        scope.validate()?;
    }
    let mut ids = HashSet::new();
    for block in &turn.content {
        match block {
            BedrockTurnBlock::Tool(call) => {
                if call.tool_use_id.trim().is_empty()
                    || call.tool_use_id.len() > 1024
                    || call.name.trim().is_empty()
                    || call.name.len() > 1024
                    || !ids.insert(&call.tool_use_id)
                {
                    return Err(invalid());
                }
            }
            BedrockTurnBlock::Reasoning(BedrockReasoning::Redacted(bytes)) => {
                // Validate only: preserve a full native JSON blob's original representation.
                STANDARD.decode(bytes).map_err(|_| invalid())?;
            }
            _ => {}
        }
    }
    Ok(())
}
