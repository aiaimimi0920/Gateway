use crate::error::GatewayError;
use crate::protocol::canonical::{
    CanonicalMessage, CanonicalRelayRequest, ContentPart, MessageRole,
};
use crate::protocol::tool_choice::{self, CanonicalToolChoice};
use crate::protocol::tool_inject;

pub fn prompt_from_request(
    req: &CanonicalRelayRequest,
    missing_message: &str,
    missing_code: &str,
) -> Result<String, GatewayError> {
    if let Some(prompt) = read_optional_string(&req.raw_body, &["prompt", "input", "lyrics"]) {
        if !prompt.is_empty() {
            return Ok(prompt);
        }
    }

    let prompt = req.messages_text().trim().to_string();
    if prompt.is_empty() {
        return Err(GatewayError::bad_request(missing_message).with_code(missing_code));
    }

    Ok(prompt)
}

pub(crate) fn latest_nonempty_user_text(req: &CanonicalRelayRequest) -> Option<String> {
    req.messages
        .iter()
        .rev()
        .find(|message| message.role == MessageRole::User)
        .map(render_message_content_for_prompt)
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
}

pub fn prompt_for_text_request(
    req: &CanonicalRelayRequest,
    missing_message: &str,
    missing_code: &str,
) -> Result<String, GatewayError> {
    let has_structured_conversation = !req.messages.is_empty() || !req.tools.is_empty();
    if !has_structured_conversation {
        return prompt_from_request(req, missing_message, missing_code);
    }
    let has_tool_history = req
        .messages
        .iter()
        .any(|message| !message.tool_calls.is_empty() || message.role == MessageRole::Tool);
    if !req.tools.is_empty() || has_tool_history {
        let rendered = build_direct_tool_prompt(req, has_tool_history);
        if !rendered.trim().is_empty() {
            return Ok(rendered);
        }
    }
    if req.tools.is_empty() && !has_tool_history {
        let system_text = req
            .system_message()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string);
        if let Some(user_text) = latest_nonempty_user_text(req) {
            if let Some(system) = system_text {
                if let Some(exact_answer) = extract_explicit_exact_wording(&user_text)
                    .or_else(|| extract_explicit_exact_wording(&system))
                {
                    return Ok(format!(
                        "{system}\n\nUser request:\n{user_text}\n\nReturn exactly this text and nothing else:\n{exact_answer}"
                    ));
                }
                return Ok(format!("{system}\n\nUser request:\n{user_text}"));
            }
            return Ok(user_text);
        }
        return prompt_from_request(req, missing_message, missing_code);
    }
    prompt_from_request(req, missing_message, missing_code)
}

fn render_message_content_for_prompt(message: &CanonicalMessage) -> String {
    let mut lines: Vec<String> = Vec::new();
    for part in &message.content {
        match part {
            ContentPart::Text { text } => {
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    lines.push(trimmed.to_string());
                }
            }
            ContentPart::ImageUrl { image_url, detail } => {
                let suffix = detail
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(|value| format!(" (detail: {value})"))
                    .unwrap_or_default();
                lines.push(format!("[image] {image_url}{suffix}"));
            }
            ContentPart::Json { value } | ContentPart::Raw { value } => {
                lines.push(value.to_string());
            }
        }
    }

    if lines.is_empty() {
        let fallback = message.text_content();
        let trimmed = fallback.trim();
        if !trimmed.is_empty() {
            lines.push(trimmed.to_string());
        }
    }

    lines.join("\n")
}

fn detect_explicit_tool_name<'a>(
    req: &'a CanonicalRelayRequest,
    latest_user_text: &str,
) -> Option<&'a str> {
    let lowered = latest_user_text.to_ascii_lowercase();
    let mut matches = req
        .tools
        .iter()
        .filter_map(|tool| tool.name.as_deref())
        .filter(|name| {
            let lowered_name = name.to_ascii_lowercase();
            lowered.contains(&format!("`{lowered_name}`")) || lowered.contains(&lowered_name)
        })
        .collect::<Vec<_>>();
    matches.sort_unstable();
    matches.dedup();
    if matches.len() == 1 {
        matches.into_iter().next()
    } else {
        None
    }
}

fn extract_explicit_exact_wording(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }

    let markers = [
        "reply with exactly:",
        "return exactly:",
        "output exactly:",
        "respond with exactly:",
    ];
    let lowered = trimmed.to_ascii_lowercase();
    for marker in markers {
        if let Some(index) = lowered.find(marker) {
            let remainder = trimmed[index + marker.len()..].trim();
            if remainder.is_empty() {
                continue;
            }
            let candidate = remainder
                .lines()
                .find_map(|line| {
                    let value = line.trim().trim_matches('`').trim_matches('"').trim();
                    if value.is_empty() {
                        None
                    } else {
                        Some(value.to_string())
                    }
                })
                .filter(|value| !value.is_empty());
            if candidate.is_some() {
                return candidate;
            }
        }
    }

    None
}

fn build_direct_tool_prompt(req: &CanonicalRelayRequest, has_tool_history: bool) -> String {
    let mut sections = Vec::new();
    let system_text = req
        .system_message()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string);
    if let Some(system) = system_text.as_deref() {
        sections.push(system.to_string());
    }

    if !req.tools.is_empty() {
        let mut instruction = String::from(
            "Use ONLY caller-provided tools. Never use Google Weather, Google Search, Google Calendar, browser tools, or any hidden Gemini tool. Do not mention transcripts, browser state, or protocol bridges.",
        );
        match tool_choice::parse_tool_choice(req.tool_choice.as_ref())
            .unwrap_or(CanonicalToolChoice::Auto)
        {
            CanonicalToolChoice::Required => {
                instruction.push_str(
                    " Tool use is required. Output ONLY a single <tool_calls>...</tool_calls> block and nothing else.",
                );
            }
            CanonicalToolChoice::Specific(ref name) => {
                instruction.push_str(
                    " Tool use is required. Output ONLY a single <tool_calls>...</tool_calls> block and nothing else.",
                );
                instruction.push_str(&format!(" The only allowed tool name is `{name}`."));
            }
            CanonicalToolChoice::PromptOnly
            | CanonicalToolChoice::Auto
            | CanonicalToolChoice::None => {}
        }
        sections.push(instruction);
        sections.push(
            "Read the user's request literally. Preserve any city names, IDs, and parameters exactly as written by the user. Do not invent extra locations, arguments, or extra tool calls. If the user says to use only one tool, emit exactly one tool call.".to_string(),
        );
        sections.push(tool_inject::build_tool_injection_prompt(
            &req.tools,
            req.tool_choice.as_ref(),
        ));
        if let Some(user_text) = latest_nonempty_user_text(req) {
            if let Some(explicit_tool_name) = detect_explicit_tool_name(req, &user_text) {
                sections.push(format!(
                    "The user explicitly named tool `{explicit_tool_name}`. You must call exactly `{explicit_tool_name}` and no other tool."
                ));
            }
            sections.push(format!("User request:\n{user_text}"));
        }
        return sections.join("\n\n");
    }

    if has_tool_history {
        sections.push(
            "The caller has already executed every required external tool. Do NOT call any tool again. Do NOT use Google Weather, Google Search, Google Calendar, browser tools, or any hidden Gemini tool. Treat the caller-provided tool result as authoritative. If the system instruction requests exact wording, output that exact wording verbatim.".to_string(),
        );
        sections.push(
            "Do not emit planning text, conflict-resolution text, or any internal meta commentary. If exact wording is requested, return only that exact wording and no additional explanation, tool recap, or factual restatement.".to_string(),
        );

        for message in &req.messages {
            if message.role == MessageRole::Tool {
                let tool_name = req
                    .messages
                    .iter()
                    .filter(|candidate| candidate.role == MessageRole::Assistant)
                    .flat_map(|candidate| candidate.tool_calls.iter())
                    .find(|tool_call| tool_call.id.as_deref() == message.tool_call_id.as_deref())
                    .and_then(|tool_call| tool_call.name.as_deref())
                    .unwrap_or("tool");
                let content = render_message_content_for_prompt(message);
                if !content.trim().is_empty() {
                    sections.push(format!(
                        "Caller-provided result for `{tool_name}`:\n{content}"
                    ));
                }
            }
        }

        if let Some(user_text) = latest_nonempty_user_text(req) {
            sections.push(format!("Final user instruction:\n{user_text}"));
        }
        let exact_answer = system_text
            .as_deref()
            .and_then(extract_explicit_exact_wording)
            .or_else(|| {
                latest_nonempty_user_text(req)
                    .as_deref()
                    .and_then(extract_explicit_exact_wording)
            });
        if let Some(exact_answer) = exact_answer {
            sections.push(format!(
                "Required exact final answer:\n{exact_answer}\n\nReturn exactly that text and nothing else."
            ));
        }
        sections.push(
            "Return only the final assistant answer requested by the caller. Do not restate the tool call history and do not rewrite the tool result unless the final user instruction explicitly asks for a summary.".to_string(),
        );
        return sections.join("\n\n");
    }

    String::new()
}

fn read_optional_string(value: &serde_json::Value, keys: &[&str]) -> Option<String> {
    let map = value.as_object()?;
    for key in keys {
        if let Some(value) = map.get(*key).and_then(serde_json::Value::as_str) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}
