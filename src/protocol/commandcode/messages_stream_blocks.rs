//! Validate known block deltas without reconstructing or retaining native extension content.
use crate::protocol::stream_decode::MAX_TRANSLATED_SSE_FRAME_BYTES;
use serde_json::Value;

pub(super) enum Block {
    Text,
    Thinking {
        signed: bool,
    },
    Tool {
        initial_nonempty: bool,
        arguments: String,
    },
    Whole,
    Opaque,
}

impl Block {
    pub(super) fn start(value: &Value) -> Result<Self, ()> {
        match value["type"].as_str().filter(|s| !s.is_empty()).ok_or(())? {
            "text" if value["text"].is_string() => Ok(Self::Text),
            "thinking" if value["thinking"].is_string() => {
                if !value["signature"].is_null() && !value["signature"].is_string() {
                    return Err(());
                }
                Ok(Self::Thinking {
                    signed: value["signature"].as_str().is_some_and(|s| !s.is_empty()),
                })
            }
            "tool_use" | "server_tool_use"
                if value["id"].as_str().is_some_and(|s| !s.is_empty())
                    && value["name"].as_str().is_some_and(|s| !s.is_empty())
                    && value["input"].is_object() =>
            {
                Ok(Self::Tool {
                    initial_nonempty: !value["input"].as_object().unwrap().is_empty(),
                    arguments: String::new(),
                })
            }
            "redacted_thinking" if value["data"].as_str().is_some_and(|s| !s.is_empty()) => {
                Ok(Self::Whole)
            }
            "text" | "thinking" | "tool_use" | "server_tool_use" | "redacted_thinking" => Err(()),
            kind if kind.ends_with("_tool_result") => {
                if value["tool_use_id"].as_str().is_some_and(|s| !s.is_empty())
                    && (value["content"].is_array() || value["content"].is_object())
                {
                    Ok(Self::Whole)
                } else {
                    Err(())
                }
            }
            _ => Ok(Self::Opaque),
        }
    }

    pub(super) fn delta(&mut self, delta: &Value, argument_bytes: &mut usize) -> Result<(), ()> {
        match (self, delta["type"].as_str()) {
            (Self::Text, Some("text_delta")) if delta["text"].is_string() => Ok(()),
            (Self::Text, Some("citations_delta")) if delta["citation"].is_object() => Ok(()),
            (Self::Thinking { signed: false }, Some("thinking_delta"))
                if delta["thinking"].is_string() =>
            {
                Ok(())
            }
            (Self::Thinking { signed }, Some("signature_delta"))
                if delta["signature"].as_str().is_some_and(|s| !s.is_empty()) =>
            {
                *signed = true;
                Ok(())
            }
            (
                Self::Tool {
                    initial_nonempty,
                    arguments,
                },
                Some("input_json_delta"),
            ) => {
                let fragment = delta["partial_json"].as_str().ok_or(())?;
                if *initial_nonempty && !fragment.is_empty() {
                    return Err(());
                }
                // The budget is shared by all open tools and released when each block closes.
                *argument_bytes = argument_bytes
                    .checked_add(fragment.len())
                    .filter(|n| *n <= MAX_TRANSLATED_SSE_FRAME_BYTES)
                    .ok_or(())?;
                arguments.push_str(fragment);
                Ok(())
            }
            (Self::Opaque, Some(kind)) if !kind.is_empty() => Ok(()),
            _ => Err(()),
        }
    }

    pub(super) fn finish(self, argument_bytes: &mut usize) -> Result<(), ()> {
        match self {
            Self::Thinking { signed: false } => Err(()),
            Self::Tool { arguments, .. } => {
                *argument_bytes -= arguments.len();
                if !arguments.is_empty()
                    && !crate::protocol::unique_json::parse(arguments.as_bytes())
                        .map_err(|_| ())?
                        .is_object()
                {
                    return Err(());
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }
}
