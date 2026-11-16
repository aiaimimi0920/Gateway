//! Optional bounded native snapshot. Text-only streams retain their existing larger body budget.
use super::{
    headers::Event,
    invalid,
    reasoning::Delta,
    state::{index, Update},
};
use crate::protocol::{
    bedrock_turn::{
        self, BedrockAssistantTurn, BedrockReasoning, BedrockReasoningText, BedrockToolUse,
        BedrockTurnBlock,
    },
    stream_error::ProtocolStreamError,
};
use base64::{engine::general_purpose::STANDARD, Engine};
use std::collections::BTreeMap;

enum Part {
    Text(String),
    Tool {
        id: String,
        name: String,
        arguments: String,
    },
    Reasoning {
        text: String,
        signature: Option<String>,
    },
    Redacted(Vec<u8>),
}
#[derive(Default)]
pub(super) struct Snapshot {
    blocks: BTreeMap<u32, Part>,
    retained: usize,
    dropped: bool,
    reasoning: bool,
}
impl Snapshot {
    pub fn accept(&mut self, event: &Event, update: &Update) -> Result<(), ProtocolStreamError> {
        if matches!(update, Update::Start) {
            return Ok(());
        }
        self.reasoning |= matches!(update, Update::Reasoning(_));
        let bytes = match update {
            Update::Text(s) | Update::ToolDelta { input: s, .. } => s.len(),
            Update::ToolStart { id, name, .. } => id.len() + name.len(),
            Update::Reasoning(Delta::Text(s) | Delta::Signature(s)) => s.len(),
            Update::Reasoning(Delta::Redacted(s)) => s.len(),
            Update::Start => unreachable!(),
        };
        self.retained = self.retained.saturating_add(bytes);
        if self.retained > bedrock_turn::MAX_TURN_BYTES || self.dropped {
            self.blocks.clear();
            self.dropped = true;
            // A later reasoning block cannot resurrect a truncated conversation snapshot.
            return if self.reasoning {
                Err(invalid("reasoning response exceeds retained-byte limit"))
            } else {
                Ok(())
            };
        }
        let index = index(&event.body)?;
        match update {
            Update::Text(text) => {
                let part = self
                    .blocks
                    .entry(index)
                    .or_insert_with(|| Part::Text(String::new()));
                let Part::Text(value) = part else {
                    return Err(invalid("snapshot text identity conflict"));
                };
                value.push_str(text);
            }
            Update::ToolStart { id, name, .. } => {
                self.blocks.insert(
                    index,
                    Part::Tool {
                        id: id.clone(),
                        name: name.clone(),
                        arguments: String::new(),
                    },
                );
            }
            Update::ToolDelta { input, .. } => {
                let Some(Part::Tool { arguments, .. }) = self.blocks.get_mut(&index) else {
                    return Err(invalid("snapshot tool identity conflict"));
                };
                arguments.push_str(input);
            }
            Update::Reasoning(delta) => {
                let part = self.blocks.entry(index).or_insert_with(|| match delta {
                    Delta::Redacted(_) => Part::Redacted(Vec::new()),
                    _ => Part::Reasoning {
                        text: String::new(),
                        signature: None,
                    },
                });
                match (part, delta) {
                    (Part::Reasoning { text, .. }, Delta::Text(value)) => text.push_str(value),
                    (Part::Reasoning { signature, .. }, Delta::Signature(value)) => {
                        signature.get_or_insert_with(String::new).push_str(value)
                    }
                    (Part::Redacted(bytes), Delta::Redacted(value)) => {
                        bytes.extend_from_slice(value)
                    }
                    _ => return Err(invalid("snapshot reasoning identity conflict")),
                }
            }
            Update::Start => unreachable!(),
        }
        Ok(())
    }

    pub fn finish(self) -> Result<Option<BedrockAssistantTurn>, ProtocolStreamError> {
        if !self.reasoning {
            return Ok(None);
        }
        let mut content = Vec::with_capacity(self.blocks.len());
        for part in self.blocks.into_values() {
            content.push(match part {
                Part::Text(text) => BedrockTurnBlock::Text(text),
                Part::Tool {
                    id,
                    name,
                    arguments,
                } => BedrockTurnBlock::Tool(BedrockToolUse {
                    tool_use_id: id,
                    name,
                    input: serde_json::from_str(&arguments)
                        .map_err(|_| invalid("invalid snapshot tool JSON"))?,
                }),
                Part::Reasoning { text, signature } => {
                    BedrockTurnBlock::Reasoning(BedrockReasoning::Text(BedrockReasoningText {
                        text,
                        signature,
                    }))
                }
                Part::Redacted(bytes) => {
                    BedrockTurnBlock::Reasoning(BedrockReasoning::Redacted(STANDARD.encode(bytes)))
                }
            });
        }
        let turn = BedrockAssistantTurn {
            content,
            continuation: None,
        };
        bedrock_turn::wire::validate(&turn)
            .map_err(|_| invalid("invalid or oversized reasoning turn"))?;
        Ok(Some(turn))
    }
}
