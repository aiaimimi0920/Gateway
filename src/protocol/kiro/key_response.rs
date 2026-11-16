//! CLI Key/OAuth runtime integrity; legacy REST keeps its historical permissive EOF contract.
use super::event_stream::KiroEvent;
use crate::error::GatewayError;
use std::collections::HashMap;

const MAX_TOOLS: usize = 4096;
const MAX_RETAINED_BYTES: usize = 64 * 1024 * 1024;

#[derive(Debug, Default)]
pub(super) struct KeyResponse {
    output_seen: bool,
    tools: HashMap<String, Tool>,
    retained: usize,
}
#[derive(Debug)]
struct Tool {
    name: String,
    arguments: String,
    nonblank_arguments: bool,
    stopped: bool,
}

impl KeyResponse {
    pub(super) fn observe(&mut self, event: &mut KiroEvent) -> Result<(), GatewayError> {
        match event {
            KiroEvent::AssistantResponse { content } => self.output_seen |= !content.is_empty(),
            KiroEvent::ToolUse {
                name,
                tool_use_id,
                input,
                stop,
            } => {
                if tool_use_id.trim().is_empty() {
                    return Err(invalid());
                }
                if !self.tools.contains_key(tool_use_id) {
                    if name.trim().is_empty() || self.tools.len() >= MAX_TOOLS {
                        return Err(invalid());
                    }
                    self.reserve_bytes(
                        tool_use_id
                            .len()
                            .checked_add(name.len())
                            .ok_or_else(invalid)?,
                    )?;
                    self.tools.try_reserve(1).map_err(|_| invalid())?;
                    self.tools.insert(
                        tool_use_id.clone(),
                        Tool {
                            name: name.clone(),
                            arguments: String::new(),
                            nonblank_arguments: false,
                            stopped: false,
                        },
                    );
                }
                let tool = self.tools.get(tool_use_id).expect("admitted Key tool");
                if tool.stopped || (!name.is_empty() && name != &tool.name) {
                    return Err(invalid());
                }
                self.reserve_bytes(input.len())?;
                let tool = self.tools.get_mut(tool_use_id).expect("admitted Key tool");
                tool.arguments
                    .try_reserve(input.len())
                    .map_err(|_| invalid())?;
                tool.arguments.push_str(input);
                // Inspect each delta once, not the growing prefix on every frame.
                tool.nonblank_arguments |= !input.trim().is_empty();
                if !tool.nonblank_arguments {
                    // No JSON token has started. Suppress leading blank deltas until stop; then
                    // all public encoders receive the same empty object as buffered accumulation.
                    input.clear();
                    if *stop {
                        input.try_reserve(2).map_err(|_| invalid())?;
                        input.push_str("{}");
                    }
                }
                if *stop {
                    if tool.nonblank_arguments {
                        let value: serde_json::Value =
                            serde_json::from_str(&tool.arguments).map_err(|_| invalid())?;
                        if !value.is_object() {
                            return Err(invalid());
                        }
                    }
                    // Keep bounded identity/closed markers, but release argument storage at stop.
                    self.retained -= tool.arguments.len();
                    tool.arguments = String::new();
                    tool.stopped = true;
                    self.output_seen = true;
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn reserve_bytes(&mut self, bytes: usize) -> Result<(), GatewayError> {
        self.retained = self
            .retained
            .checked_add(bytes)
            .filter(|total| *total <= MAX_RETAINED_BYTES)
            .ok_or_else(invalid)?;
        Ok(())
    }

    pub(super) fn finish(&self) -> Result<(), GatewayError> {
        if !self.output_seen || self.tools.values().any(|tool| !tool.stopped) {
            return Err(invalid());
        }
        Ok(())
    }
}

pub(super) fn invalid() -> GatewayError {
    GatewayError::server_error("Invalid or incomplete Kiro CLI Key response.")
        .with_code("kiro_api_key_invalid_stream")
}

#[cfg(test)]
mod tests {
    use super::*;
    fn tool(id: String) -> KiroEvent {
        KiroEvent::ToolUse {
            name: "read".into(),
            tool_use_id: id,
            input: "{}".into(),
            stop: true,
        }
    }
    #[test]
    fn key_response_bounds_metadata_count_and_checked_retained_arithmetic() {
        let mut state = KeyResponse::default();
        for i in 0..MAX_TOOLS {
            state.observe(&mut tool(i.to_string())).unwrap();
        }
        assert!(state.observe(&mut tool("overflow".into())).is_err());
        for retained in [MAX_RETAINED_BYTES, usize::MAX] {
            let mut state = KeyResponse {
                retained,
                ..Default::default()
            };
            assert!(state.observe(&mut tool("a".into())).is_err());
        }
    }
}
