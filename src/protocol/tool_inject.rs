// ---------------------------------------------------------------------------
// protocol/tool_inject.rs — XML tool call injection and extraction
//
// Enables models without native tool-calling support (DeepSeek, small models)
// to work with tool-calling clients by:
//   1. Injecting XML tool definitions into the system prompt
//   2. Stripping the `tools` field from the request
//   3. Parsing XML tool calls from the model's text response
//   4. Converting them back to standard OpenAI `tool_calls` format
// ---------------------------------------------------------------------------

#[cfg(test)]
use std::collections::HashMap;

#[cfg(test)]
use bytes::Bytes;
#[cfg(test)]
use serde_json::Value;

use crate::protocol::canonical::{
    CanonicalMessage, CanonicalRelayRequest, ContentPart, MessageRole,
};
#[cfg(test)]
use crate::protocol::canonical::{CanonicalTool, CanonicalToolCall};

mod history;
mod parser;
mod prompt;
mod streaming;
pub use history::serialize_tool_history;
#[cfg(test)]
use history::{format_tool_call_as_xml, xml_escape};
#[cfg(test)]
use parser::strip_think_blocks;
pub use parser::{
    parse_tool_calls_from_text, parse_tool_calls_from_text_with_context, ToolCallParseResult,
};
pub use prompt::build_tool_injection_prompt;
pub use streaming::wrap_streaming_tool_detection;
#[cfg(test)]
use streaming::{
    build_tool_calls_sse_chunk, extract_sse_content, has_tool_call_opening,
    wrap_streaming_tool_detection_with_limits,
};

// ---------------------------------------------------------------------------
// Part 2: Inject tools into request
// ---------------------------------------------------------------------------

/// Inject tool definitions into a [`CanonicalRelayRequest`].
///
/// - Prepends the XML tool prompt to the system message (or creates one)
/// - Converts previous `tool_call` messages in conversation history to XML
///   format so the model understands context
/// - Converts `tool` (result) messages to text format
/// - Clears the `tools` field (model does not understand native tool format)
pub fn inject_tools(req: &mut CanonicalRelayRequest) {
    if req.tools.is_empty() {
        serialize_tool_history(req);
        return;
    }

    let tool_prompt = build_tool_injection_prompt(&req.tools, req.tool_choice.as_ref());

    // Prepend to existing system message or create new one.
    let has_system = req.messages.iter().any(|m| m.role == MessageRole::System);

    if has_system {
        for msg in &mut req.messages {
            if msg.role == MessageRole::System {
                let existing_text = msg.text_content();
                msg.content = vec![ContentPart::Text {
                    text: format!("{}\n\n{}", tool_prompt, existing_text),
                }];
                break;
            }
        }
    } else {
        req.messages.insert(
            0,
            CanonicalMessage {
                role: MessageRole::System,
                content: vec![ContentPart::Text { text: tool_prompt }],
                name: None,
                tool_call_id: None,
                tool_calls: vec![],
            },
        );
    }

    serialize_tool_history(req);

    // Clear native tools (model doesn't understand them).
    req.tools.clear();
    req.tool_choice = None;
}

// ---------------------------------------------------------------------------
// Part 4: Model detection
// ---------------------------------------------------------------------------

/// Check if a model needs XML tool injection (doesn't support native tool calling).
///
/// Models that DO support native tools (no injection needed):
/// - `gpt-*` (OpenAI)
/// - `claude-*` (Anthropic)
/// - `o1-*`, `o3-*`, `o4-*` (OpenAI reasoning)
/// - Any model accessed via adapters with native tool support:
///   - `anthropic_compatible`
///   - `kiro_compatible`
///   - `gemini_api_compatible`
///   - `aistudio_web_reverse_compatible`
///   - `gemini_canvas_compatible`
///   - `bedrock_converse_compatible`
///   - `cohere_compatible`
///
/// Models that NEED injection:
/// - `deepseek-*`
/// - Any model not in the native-tools list
pub fn needs_tool_injection(model: &str, adapter: &str) -> bool {
    if adapter == "chatgpt_web_reverse_compatible" {
        return true;
    }

    // Native-tool adapters should never downgrade to XML tool injection.
    if matches!(
        adapter,
        "anthropic_compatible"
            | "kiro_compatible"
            | "gemini_api_compatible"
            | "gemini_api_modular_compatible"
            | "aistudio_web_reverse_compatible"
            | "gemini_canvas_compatible"
            | "gemini_canvas_web_reverse_compatible"
            | "bedrock_converse_compatible"
            | "cohere_compatible"
    ) {
        return false;
    }

    let model_lower = model.to_lowercase();

    // Models with native tool support — no injection needed.
    let native_prefixes = ["gpt-", "o1-", "o3-", "o4-", "claude-"];

    // Exact matches for short model names.
    let native_exact = ["o1", "o3", "o4"];

    for exact in &native_exact {
        if model_lower == *exact {
            return false;
        }
    }

    for prefix in &native_prefixes {
        if model_lower.starts_with(prefix) {
            return false;
        }
    }

    // Everything else needs injection.
    true
}

#[cfg(test)]
mod tests;
