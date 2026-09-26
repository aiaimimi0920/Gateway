//! Decode wrapped and typed streaming envelopes before complete-response fallbacks.

use serde_json::Value;

use super::{map_finish_reason, usage_from_value, value_to_string, ParsedEvent};

// A string type consumes the envelope even when the event name is unrecognized.
pub(super) fn parse_stream_event(raw: &Value, events: &mut Vec<ParsedEvent>) -> bool {
    if let Some(event) = raw.get("messageStart") {
        events.push(ParsedEvent::Start {
            model: event
                .get("model")
                .and_then(|v| v.as_str())
                .map(str::to_string),
            usage: event.get("usage").and_then(|v| usage_from_value(Some(v))),
        });
        return true;
    }

    if let Some(event) = raw.get("contentBlockStart") {
        if let Some(tool_use) = event
            .get("start")
            .and_then(|v| v.get("toolUse"))
            .or_else(|| event.get("toolUse"))
        {
            events.push(ParsedEvent::ToolStart {
                index: event
                    .get("contentBlockIndex")
                    .or_else(|| event.get("index"))
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0),
                id: tool_use
                    .get("toolUseId")
                    .or_else(|| tool_use.get("id"))
                    .and_then(value_to_string)
                    .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
                name: tool_use
                    .get("name")
                    .and_then(value_to_string)
                    .unwrap_or_default(),
            });
        }
        return true;
    }

    if let Some(event) = raw.get("contentBlockDelta") {
        let index = event
            .get("contentBlockIndex")
            .or_else(|| event.get("index"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        if let Some(text) = event
            .get("delta")
            .and_then(|v| v.get("text"))
            .and_then(|v| v.as_str())
        {
            events.push(ParsedEvent::Text(text.to_string()));
        }
        if let Some(partial) = event
            .get("delta")
            .and_then(|v| {
                v.get("partial_json")
                    .or_else(|| v.get("partialJson"))
                    .or_else(|| v.get("input"))
                    .or_else(|| v.get("toolUse").and_then(|entry| entry.get("input")))
            })
            .and_then(|v| v.as_str())
        {
            events.push(ParsedEvent::ToolDelta {
                index,
                partial: partial.to_string(),
            });
        }
        return true;
    }

    if let Some(event) = raw.get("contentBlockStop") {
        events.push(ParsedEvent::ToolEnd {
            index: event
                .get("contentBlockIndex")
                .or_else(|| event.get("index"))
                .and_then(|v| v.as_i64())
                .unwrap_or(0),
        });
        return true;
    }

    if let Some(event) = raw.get("messageStop") {
        events.push(ParsedEvent::Finish {
            reason: map_finish_reason(
                event
                    .get("stopReason")
                    .or_else(|| event.get("stop_reason"))
                    .and_then(|v| v.as_str()),
            ),
            usage: None,
        });
        return true;
    }

    if let Some(event) = raw.get("metadata") {
        events.push(ParsedEvent::Finish {
            reason: None,
            usage: event.get("usage").and_then(|v| usage_from_value(Some(v))),
        });
        return true;
    }

    if let Some(event_type) = raw.get("type").and_then(|v| v.as_str()) {
        match event_type {
            "message_start" | "messageStart" => {
                events.push(ParsedEvent::Start {
                    model: raw
                        .get("message")
                        .or_else(|| raw.get("delta").and_then(|v| v.get("message")))
                        .and_then(|v| v.get("model"))
                        .and_then(|v| v.as_str())
                        .map(str::to_string),
                    usage: raw
                        .get("message")
                        .or_else(|| raw.get("delta").and_then(|v| v.get("message")))
                        .and_then(|v| v.get("usage"))
                        .and_then(|v| usage_from_value(Some(v))),
                });
            }
            "content_block_start" | "contentBlockStart" => {
                if let Some(block) = raw
                    .get("content_block")
                    .or_else(|| raw.get("contentBlock"))
                    .or_else(|| raw.get("start"))
                {
                    let tool_use = if block.get("type").and_then(|v| v.as_str()) == Some("tool_use")
                    {
                        Some(block)
                    } else {
                        block.get("toolUse")
                    };
                    if let Some(tool_use) = tool_use {
                        events.push(ParsedEvent::ToolStart {
                            index: raw.get("index").and_then(|v| v.as_i64()).unwrap_or(0),
                            id: tool_use
                                .get("id")
                                .or_else(|| tool_use.get("toolUseId"))
                                .and_then(value_to_string)
                                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
                            name: tool_use
                                .get("name")
                                .and_then(value_to_string)
                                .unwrap_or_default(),
                        });
                    }
                }
            }
            "content_block_delta" | "contentBlockDelta" => {
                if let Some(delta) = raw.get("delta") {
                    let mut handled_text = false;
                    let mut handled_tool_delta = false;
                    match delta.get("type").and_then(|v| v.as_str()).unwrap_or("") {
                        "text_delta" => {
                            if let Some(text) = delta.get("text").and_then(|v| v.as_str()) {
                                events.push(ParsedEvent::Text(text.to_string()));
                                handled_text = true;
                            }
                        }
                        "input_json_delta" => {
                            if let Some(partial) =
                                delta.get("partial_json").and_then(|v| v.as_str())
                            {
                                events.push(ParsedEvent::ToolDelta {
                                    index: raw.get("index").and_then(|v| v.as_i64()).unwrap_or(0),
                                    partial: partial.to_string(),
                                });
                                handled_tool_delta = true;
                            }
                        }
                        _ => {}
                    }
                    if !handled_text {
                        if let Some(text) = delta
                            .get("text")
                            .or_else(|| delta.get("delta").and_then(|v| v.get("text")))
                            .and_then(|v| v.as_str())
                        {
                            events.push(ParsedEvent::Text(text.to_string()));
                        }
                    }
                    if !handled_tool_delta {
                        if let Some(partial) = delta
                            .get("toolUse")
                            .and_then(|v| v.get("input"))
                            .and_then(|v| v.as_str())
                        {
                            events.push(ParsedEvent::ToolDelta {
                                index: raw.get("index").and_then(|v| v.as_i64()).unwrap_or(0),
                                partial: partial.to_string(),
                            });
                        }
                    }
                }
            }
            "content_block_stop" | "contentBlockStop" => events.push(ParsedEvent::ToolEnd {
                index: raw.get("index").and_then(|v| v.as_i64()).unwrap_or(0),
            }),
            "message_delta" | "messageDelta" => events.push(ParsedEvent::Finish {
                reason: map_finish_reason(
                    raw.get("delta")
                        .and_then(|v| v.get("stop_reason").or_else(|| v.get("stopReason")))
                        .and_then(|v| v.as_str()),
                ),
                usage: raw
                    .get("usage")
                    .or_else(|| raw.get("delta").and_then(|v| v.get("usage")))
                    .and_then(|v| usage_from_value(Some(v))),
            }),
            "message_stop" | "messageStop" => events.push(ParsedEvent::Done),
            _ => {}
        }
        return true;
    }

    false
}
