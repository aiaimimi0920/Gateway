use serde_json::Value;

use super::stream_decode::ParsedEvent;
use super::{map_finish_reason, parse_provider_error, usage_from_value, value_to_string};

// The disabled facade also loads this file through #[path]; anchor both children.
#[path = "event_parse/stream_events.rs"]
mod stream_events;

#[cfg(test)]
#[path = "event_parse/tests.rs"]
mod tests;

pub(super) fn parse_raw_event(raw: &Value) -> Vec<ParsedEvent> {
    let mut events = Vec::new();
    if let Some(error) = parse_provider_error(raw) {
        events.push(error);
        return events;
    }

    if stream_events::parse_stream_event(raw, &mut events) {
        return events;
    }

    if let Some(model) = raw
        .get("modelVersion")
        .or_else(|| raw.get("model"))
        .and_then(|v| v.as_str())
    {
        events.push(ParsedEvent::Start {
            model: Some(model.to_string()),
            usage: None,
        });
    }

    if let Some(candidates) = raw.get("candidates").and_then(|v| v.as_array()) {
        if let Some(candidate) = candidates.first() {
            if let Some(parts) = candidate
                .get("content")
                .and_then(|v| v.get("parts"))
                .and_then(|v| v.as_array())
            {
                for part in parts {
                    if let Some(text) = part.get("text").and_then(|v| v.as_str()) {
                        events.push(ParsedEvent::Text(text.to_string()));
                    }
                    if let Some(function_call) = part
                        .get("functionCall")
                        .or_else(|| part.get("function_call"))
                        .and_then(|v| v.as_object())
                    {
                        events.push(ParsedEvent::ToolCall {
                            id: function_call
                                .get("id")
                                .and_then(value_to_string)
                                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
                            name: function_call
                                .get("name")
                                .and_then(value_to_string)
                                .unwrap_or_default(),
                            arguments: function_call
                                .get("argsJson")
                                .or_else(|| function_call.get("args"))
                                .map(|v| {
                                    if let Some(text) = v.as_str() {
                                        text.to_string()
                                    } else {
                                        serde_json::to_string(v).unwrap_or_else(|_| "{}".into())
                                    }
                                })
                                .unwrap_or_else(|| "{}".into()),
                        });
                    }
                }
            }
            events.push(ParsedEvent::Finish {
                reason: map_finish_reason(
                    candidate
                        .get("finishReason")
                        .or_else(|| raw.get("finishReason"))
                        .and_then(|v| v.as_str()),
                ),
                usage: raw
                    .get("usageMetadata")
                    .and_then(|v| usage_from_value(Some(v))),
            });
        }
        return events;
    }

    if let Some(output_message) = raw
        .get("output")
        .and_then(|v| v.get("message"))
        .and_then(|v| v.as_object())
    {
        if let Some(content) = output_message.get("content").and_then(|v| v.as_array()) {
            for block in content {
                if let Some(text) = block.get("text").and_then(|v| v.as_str()) {
                    events.push(ParsedEvent::Text(text.to_string()));
                }
                if let Some(tool_use) = block.get("toolUse").and_then(|v| v.as_object()) {
                    events.push(ParsedEvent::ToolCall {
                        id: tool_use
                            .get("toolUseId")
                            .and_then(value_to_string)
                            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
                        name: tool_use
                            .get("name")
                            .and_then(value_to_string)
                            .unwrap_or_default(),
                        arguments: tool_use
                            .get("input")
                            .map(|v| {
                                if let Some(text) = v.as_str() {
                                    text.to_string()
                                } else {
                                    serde_json::to_string(v).unwrap_or_else(|_| "{}".into())
                                }
                            })
                            .unwrap_or_else(|| "{}".into()),
                    });
                }
            }
        }
        events.push(ParsedEvent::Finish {
            reason: map_finish_reason(raw.get("stopReason").and_then(|v| v.as_str())),
            usage: raw.get("usage").and_then(|v| usage_from_value(Some(v))),
        });
        return events;
    }

    if let Some(message) = raw.get("message").and_then(|v| v.as_object()) {
        if let Some(content) = message.get("content").and_then(|v| v.as_array()) {
            for block in content {
                if let Some(text) = block.get("text").and_then(|v| v.as_str()) {
                    events.push(ParsedEvent::Text(text.to_string()));
                }
            }
        } else if let Some(text) = message.get("text").and_then(|v| v.as_str()) {
            events.push(ParsedEvent::Text(text.to_string()));
        }

        if let Some(tool_calls) = message.get("tool_calls").and_then(|v| v.as_array()) {
            for tool_call in tool_calls {
                let function = tool_call.get("function").and_then(|v| v.as_object());
                events.push(ParsedEvent::ToolCall {
                    id: tool_call
                        .get("id")
                        .and_then(value_to_string)
                        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
                    name: tool_call
                        .get("name")
                        .or_else(|| function.and_then(|f| f.get("name")))
                        .and_then(value_to_string)
                        .unwrap_or_default(),
                    arguments: tool_call
                        .get("arguments")
                        .or_else(|| tool_call.get("parameters"))
                        .or_else(|| function.and_then(|f| f.get("arguments")))
                        .or_else(|| function.and_then(|f| f.get("parameters")))
                        .map(|v| {
                            if let Some(text) = v.as_str() {
                                text.to_string()
                            } else {
                                serde_json::to_string(v).unwrap_or_else(|_| "{}".into())
                            }
                        })
                        .unwrap_or_else(|| "{}".into()),
                });
            }
        }

        events.push(ParsedEvent::Finish {
            reason: map_finish_reason(
                raw.get("finish_reason")
                    .or_else(|| message.get("finish_reason"))
                    .and_then(|v| v.as_str()),
            ),
            usage: raw.get("usage").and_then(|v| usage_from_value(Some(v))),
        });
        return events;
    }

    if let Some(choices) = raw.get("choices").and_then(|v| v.as_array()) {
        if let Some(choice) = choices.first() {
            if let Some(text) = choice
                .get("delta")
                .and_then(|v| v.get("content"))
                .and_then(|v| v.as_str())
                .or_else(|| {
                    choice
                        .get("message")
                        .and_then(|v| v.get("content"))
                        .and_then(|v| v.as_str())
                })
            {
                events.push(ParsedEvent::Text(text.to_string()));
            }
            let tool_calls = choice
                .get("delta")
                .and_then(|v| v.get("tool_calls"))
                .or_else(|| choice.get("message").and_then(|v| v.get("tool_calls")))
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            for tool_call in tool_calls {
                let Some(function) = tool_call.get("function").and_then(|v| v.as_object()) else {
                    continue;
                };
                events.push(ParsedEvent::ToolCall {
                    id: tool_call
                        .get("id")
                        .and_then(value_to_string)
                        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
                    name: function
                        .get("name")
                        .and_then(value_to_string)
                        .unwrap_or_default(),
                    arguments: function
                        .get("arguments")
                        .map(|v| {
                            if let Some(text) = v.as_str() {
                                text.to_string()
                            } else {
                                serde_json::to_string(v).unwrap_or_else(|_| "{}".into())
                            }
                        })
                        .unwrap_or_else(|| "{}".into()),
                });
            }
            events.push(ParsedEvent::Finish {
                reason: map_finish_reason(choice.get("finish_reason").and_then(|v| v.as_str())),
                usage: raw.get("usage").and_then(|v| usage_from_value(Some(v))),
            });
        }
        return events;
    }

    if let Some(tool_calls) = raw.get("tool_calls").and_then(|v| v.as_array()) {
        for tool_call in tool_calls {
            let function = tool_call.get("function").and_then(|v| v.as_object());
            events.push(ParsedEvent::ToolCall {
                id: tool_call
                    .get("id")
                    .and_then(value_to_string)
                    .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
                name: tool_call
                    .get("name")
                    .or_else(|| function.and_then(|f| f.get("name")))
                    .and_then(value_to_string)
                    .unwrap_or_default(),
                arguments: tool_call
                    .get("arguments")
                    .or_else(|| tool_call.get("parameters"))
                    .or_else(|| function.and_then(|f| f.get("arguments")))
                    .or_else(|| function.and_then(|f| f.get("parameters")))
                    .map(|v| {
                        if let Some(text) = v.as_str() {
                            text.to_string()
                        } else {
                            serde_json::to_string(v).unwrap_or_else(|_| "{}".into())
                        }
                    })
                    .unwrap_or_else(|| "{}".into()),
            });
        }
        if let Some(text) = raw.get("text").and_then(|v| v.as_str()) {
            events.push(ParsedEvent::Text(text.to_string()));
        }
        events.push(ParsedEvent::Finish {
            reason: map_finish_reason(raw.get("finish_reason").and_then(|v| v.as_str())),
            usage: raw.get("usage").and_then(|v| usage_from_value(Some(v))),
        });
        return events;
    }

    events
}
