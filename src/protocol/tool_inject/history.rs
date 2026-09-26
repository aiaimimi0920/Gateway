use crate::protocol::canonical::{
    CanonicalMessage, CanonicalRelayRequest, CanonicalToolCall, ContentPart, MessageRole,
};

/// Convert tool history into a text-only transcript for upstreams that do not
/// understand native tool wire, even when the current turn does not carry a
/// fresh `tools` array.
pub fn serialize_tool_history(req: &mut CanonicalRelayRequest) {
    let mut i = 0;
    while i < req.messages.len() {
        let role = req.messages[i].role;

        if role == MessageRole::Assistant && !req.messages[i].tool_calls.is_empty() {
            let xml = format_tool_call_as_xml(&req.messages[i].tool_calls);
            let existing = req.messages[i].text_content();
            let new_text = if existing.is_empty() {
                xml
            } else {
                format!("{}\n{}", existing, xml)
            };
            req.messages[i].content = vec![ContentPart::Text { text: new_text }];
            req.messages[i].tool_calls.clear();
        } else if role == MessageRole::Tool {
            let tool_call_id = req.messages[i].tool_call_id.clone().unwrap_or_default();
            let content_text = render_tool_result_content(&req.messages[i]);
            let formatted = format_tool_result_as_text(&tool_call_id, &content_text);

            req.messages[i].role = MessageRole::User;
            req.messages[i].content = vec![ContentPart::Text { text: formatted }];
            req.messages[i].tool_call_id = None;
        }

        i += 1;
    }
}

/// Convert tool calls from an assistant message into XML format
/// so the model understands what tools were previously called.
pub(super) fn format_tool_call_as_xml(tool_calls: &[CanonicalToolCall]) -> String {
    let mut xml = String::from("<tool_calls>\n");
    for tc in tool_calls {
        xml.push_str("<tool_call>\n");
        xml.push_str(&format!(
            "<tool_name>{}</tool_name>\n",
            xml_escape(tc.name.as_deref().unwrap_or(""))
        ));
        xml.push_str(&format!(
            "<parameters>{}</parameters>\n",
            xml_escape(tc.arguments.as_deref().unwrap_or("{}"))
        ));
        xml.push_str("</tool_call>\n");
    }
    xml.push_str("</tool_calls>");
    xml
}

/// Convert a tool result message to text format for the model.
fn format_tool_result_as_text(tool_call_id: &str, content: &str) -> String {
    format!(
        "Tool execution result (call_id: {}):\n<tool_result>\n{}\n</tool_result>",
        tool_call_id, content
    )
}

/// Escape XML special characters in text.
pub(super) fn xml_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn render_tool_result_content(message: &CanonicalMessage) -> String {
    let mut fragments = Vec::new();
    for part in &message.content {
        match part {
            ContentPart::Text { text } => {
                if !text.trim().is_empty() {
                    fragments.push(text.clone());
                }
            }
            ContentPart::Json { value } | ContentPart::Raw { value } => {
                let rendered = value
                    .get("text")
                    .and_then(|entry| entry.as_str())
                    .map(str::to_string)
                    .unwrap_or_else(|| value.to_string());
                if !rendered.trim().is_empty() {
                    fragments.push(rendered);
                }
            }
            ContentPart::ImageUrl { .. } => {}
        }
    }
    fragments.join("\n")
}
