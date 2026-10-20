//! Verify terminal completion before exposing any tool, then replay standard Responses events.
use crate::error::GatewayError;
use serde_json::{json, Value};

fn invalid() -> GatewayError {
    GatewayError::server_error("Basispoints returned an invalid or incomplete response")
        .with_code("basispoints_invalid_response")
}

pub fn decode(bytes: &[u8], sse: bool, source: &Value) -> Result<Value, GatewayError> {
    let mut body: Value = if sse {
        let text = std::str::from_utf8(bytes)
            .map_err(|_| invalid())?
            .trim_start_matches('\u{feff}');
        let mut complete = None;
        let mut data = String::new();
        for line in text.lines().chain(std::iter::once("")) {
            if line.is_empty() {
                if !data.is_empty() && data.trim() != "[DONE]" {
                    let event: Value =
                        serde_json::from_str(data.trim_end()).map_err(|_| invalid())?;
                    match event["type"].as_str() {
                        Some("response.failed" | "response.incomplete" | "error") => {
                            return Err(invalid())
                        }
                        Some("response.completed") => {
                            if complete.is_some() {
                                return Err(invalid());
                            }
                            complete = Some(event["response"].clone());
                        }
                        _ => {}
                    }
                }
                data.clear();
            } else if let Some(value) = line.strip_prefix("data:") {
                data.push_str(value.strip_prefix(' ').unwrap_or(value));
                data.push('\n');
            }
        }
        complete.ok_or_else(invalid)?
    } else {
        serde_json::from_slice(bytes).map_err(|_| invalid())?
    };
    if body["status"] != "completed" {
        return Err(invalid());
    }
    let items = body["output"].as_array().ok_or_else(invalid)?;
    let parts = items
        .iter()
        .try_fold(items.len(), |count, item| {
            count.checked_add(item["content"].as_array().map_or(0, Vec::len))
        })
        .ok_or_else(invalid)?;
    if parts > 4096 {
        return Err(invalid());
    }
    super::tools::convert_output(&mut body, source).map_err(|_| invalid())?;
    Ok(body)
}

pub fn replay(body: &Value) -> Vec<u8> {
    let mut frames = Vec::new();
    let mut sequence = 0;
    let mut emit = |mut value: Value| {
        value["sequence_number"] = json!(sequence);
        sequence += 1;
        frames.extend_from_slice(
            format!(
                "event: {}\ndata: {}\n\n",
                value["type"].as_str().unwrap_or("message"),
                value
            )
            .as_bytes(),
        );
    };
    let mut initial = body.clone();
    initial["output"] = json!([]);
    initial["status"] = json!("in_progress");
    emit(json!({"type":"response.created","response":initial}));
    if let Some(items) = body["output"].as_array() {
        for (index, item) in items.iter().enumerate() {
            let mut initial_item = item.clone();
            if item["type"] == "message" {
                initial_item["content"] = json!([]);
            } else if item["type"] == "function_call" {
                initial_item["arguments"] = json!("");
            } else if item["type"] == "custom_tool_call" {
                initial_item["input"] = json!("");
            }
            emit(
                json!({"type":"response.output_item.added","output_index":index,"item":initial_item}),
            );
            match item["type"].as_str() {
                Some("message") => {
                    if let Some(blocks) = item["content"].as_array() {
                        for (content_index, block) in blocks.iter().enumerate() {
                            if let Some(text) = block["text"].as_str() {
                                let mut empty = block.clone();
                                empty["text"] = json!("");
                                emit(
                                    json!({"type":"response.content_part.added","output_index":index,"content_index":content_index,"item_id":item["id"],"part":empty}),
                                );
                                emit(
                                    json!({"type":"response.output_text.delta","output_index":index,"content_index":content_index,"delta":text,"item_id":item["id"]}),
                                );
                                emit(
                                    json!({"type":"response.output_text.done","output_index":index,"content_index":content_index,"text":text,"item_id":item["id"]}),
                                );
                                emit(
                                    json!({"type":"response.content_part.done","output_index":index,"content_index":content_index,"item_id":item["id"],"part":block}),
                                );
                            }
                        }
                    }
                }
                Some("function_call" | "custom_tool_call") => {
                    let custom = item["type"] == "custom_tool_call";
                    emit(
                        json!({"type":if custom {"response.custom_tool_call_input.delta"} else {"response.function_call_arguments.delta"},
                        "output_index":index,"item_id":item["id"],"delta":item[if custom {"input"} else {"arguments"}]}),
                    );
                    let mut done = json!({"type":if custom {"response.custom_tool_call_input.done"} else {"response.function_call_arguments.done"},"output_index":index,"item_id":item["id"]});
                    done[if custom { "input" } else { "arguments" }] =
                        item[if custom { "input" } else { "arguments" }].clone();
                    emit(done);
                }
                _ => {}
            }
            emit(json!({"type":"response.output_item.done","output_index":index,"item":item}));
        }
    }
    emit(json!({"type":"response.completed","response":body}));
    frames.extend_from_slice(b"data: [DONE]\n\n");
    frames
}
