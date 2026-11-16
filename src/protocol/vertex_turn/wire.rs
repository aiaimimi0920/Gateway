//! Bound public opaque state before cloning; never authenticate or interpret its encrypted bytes.
pub(crate) use super::stream::TerminalTurn;
use super::*;
use base64::Engine;
use std::{collections::HashSet, io::Write};

struct Budget(usize);
impl Write for Budget {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0 = self
            .0
            .checked_sub(bytes.len())
            .ok_or_else(|| std::io::Error::other("Vertex turn budget exceeded"))?;
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
pub(crate) fn within_budget(value: &Value, limit: usize) -> Result<(), GatewayError> {
    serde_json::to_writer(Budget(limit), value).map_err(|_| invalid())
}
pub(crate) fn read_turn(value: &Value) -> Result<VertexAssistantTurn, GatewayError> {
    within_budget(value, MAX_TURN_BYTES)?;
    let parts = value
        .get("parts")
        .and_then(Value::as_array)
        .filter(|p| !p.is_empty() && p.len() <= 4096)
        .ok_or_else(invalid)?;
    // Validate signature sizes before serde allocates a second copy.
    for part in parts {
        for name in ["text", "public_id"] {
            if part.get(name).is_some_and(|v| !v.is_string()) {
                return Err(invalid());
            }
        }
        if part.get("thought").is_some_and(|v| !v.is_boolean())
            || part.get("function_call").is_some_and(|v| !v.is_object())
            || part
                .get("function_call")
                .and_then(|v| v.get("args"))
                .is_some_and(|v| !v.is_object())
        {
            return Err(invalid());
        }
        if let Some(sig) = part.get("thought_signature") {
            let sig = sig
                .as_str()
                .filter(|s| s.len() <= 1024 * 1024)
                .ok_or_else(invalid)?;
            if matches!(
                sig,
                "skip_thought_signature_validator" | "context_engineering_is_the_way_to_go"
            ) {
                return Err(invalid());
            }
            let engines = [
                base64::engine::general_purpose::STANDARD,
                base64::engine::general_purpose::STANDARD_NO_PAD,
                base64::engine::general_purpose::URL_SAFE,
                base64::engine::general_purpose::URL_SAFE_NO_PAD,
            ];
            if !engines.iter().any(|engine| engine.decode(sig).is_ok()) {
                return Err(invalid());
            }
        }
    }
    let turn: VertexAssistantTurn = serde_json::from_value(value.clone()).map_err(|_| invalid())?;
    if !crate::protocol::vertex_express::valid_model(&turn.model) {
        return Err(invalid());
    }
    let mut ids = HashSet::new();
    for part in &turn.parts {
        if part.text.is_some() == part.function_call.is_some() {
            return Err(invalid());
        }
        match (&part.function_call, &part.public_id) {
            (Some(call), Some(id))
                if !call.name.trim().is_empty()
                    && call.name.len() <= 2048
                    && !id.trim().is_empty()
                    && id.len() <= 2048
                    && ids.insert(id) => {}
            (None, None) => {}
            _ => return Err(invalid()),
        }
        // Reasoning summaries are not executable tools.
        if part.function_call.is_some() && part.thought == Some(true) {
            return Err(invalid());
        }
    }
    Ok(turn)
}
