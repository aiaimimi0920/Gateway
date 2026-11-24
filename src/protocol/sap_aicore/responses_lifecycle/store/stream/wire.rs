//! Project an explicit subset of Responses events. No upstream metadata or
//! response IDs may reach the public channel outside typed application fields.
use super::super::super::{observe, State, UpstreamId};
use super::invalid;
use crate::{error::GatewayError, protocol::sse_parse::SseFrame};
use bytes::Bytes;
use serde_json::{json, Value};

#[derive(Default)]
pub(super) struct Wire {
    id: Option<UpstreamId>,
    sequence: Option<u64>,
    terminal: bool,
    done: bool,
    background: bool,
    created: bool,
}

pub(super) enum Frame {
    Ignore,
    Delta(Bytes),
    Snapshot {
        name: &'static str,
        sequence: u64,
        id: UpstreamId,
        state: State,
        body: Value,
    },
}

impl Wire {
    pub(super) fn create(background: bool) -> Self {
        Self {
            background,
            ..Self::default()
        }
    }

    pub(super) fn resume(id: UpstreamId, cursor: Option<i32>) -> Self {
        Self {
            id: Some(id),
            sequence: cursor.map(|n| n as u64),
            created: cursor.is_some(),
            background: true,
            ..Self::default()
        }
    }

    pub(super) fn accept(&mut self, frame: SseFrame) -> Result<Frame, GatewayError> {
        if frame.data.is_empty() && frame.event_name.is_none() {
            return Ok(Frame::Ignore);
        }
        if frame.data == "[DONE]" && frame.event_name.is_none() && self.terminal && !self.done {
            self.done = true;
            return Ok(Frame::Ignore);
        }
        if self.terminal {
            return Err(invalid());
        }
        let payload =
            crate::protocol::unique_json::parse(frame.data.as_bytes()).map_err(|_| invalid())?;
        let name = payload["type"].as_str().ok_or_else(invalid)?;
        if frame.event_name.as_deref().is_some_and(|kind| kind != name)
            || !payload["error"].is_null()
        {
            return Err(invalid());
        }
        let sequence = integer(&payload, "sequence_number")?;
        if self.sequence.is_some_and(|previous| previous >= sequence) {
            return Err(invalid());
        }
        self.sequence = Some(sequence);
        let snapshot = match name {
            "response.created" if !self.created => Some((
                "response.created",
                if self.background && payload["response"]["status"] == "queued" {
                    "queued"
                } else {
                    "in_progress"
                },
            )),
            "response.queued" if self.background && self.id.is_some() => {
                Some(("response.queued", "queued"))
            }
            "response.in_progress" if self.id.is_some() => {
                Some(("response.in_progress", "in_progress"))
            }
            "response.completed" => Some(("response.completed", "completed")),
            "response.incomplete" => Some(("response.incomplete", "incomplete")),
            "response.failed" => Some(("response.failed", "failed")),
            "response.cancelled" => Some(("response.cancelled", "cancelled")),
            _ => None,
        };
        if let Some((name, status)) = snapshot {
            let body = payload["response"].clone();
            let (id, state) = observe(&body, self.id.as_ref())?;
            if body["status"] != status || (self.id.is_none() && name != "response.created") {
                return Err(invalid());
            }
            self.id = Some(id.clone());
            self.created = true;
            self.terminal = !matches!(state, State::Pending);
            return Ok(Frame::Snapshot {
                name,
                sequence,
                id,
                state,
                body,
            });
        }
        if self.id.is_none() || !payload["response_id"].is_null() {
            return Err(invalid());
        }
        self.created = true;
        Ok(Frame::Delta(project(name, sequence, &payload)?))
    }
}

pub(super) fn event(name: &str, value: &Value) -> Bytes {
    Bytes::from(format!("event: {name}\ndata: {value}\n\n"))
}

fn integer(payload: &Value, key: &str) -> Result<u64, GatewayError> {
    payload[key].as_u64().ok_or_else(invalid)
}

fn text<'a>(payload: &'a Value, key: &str) -> Result<&'a str, GatewayError> {
    payload[key].as_str().ok_or_else(invalid)
}

fn project(name: &str, seq: u64, value: &Value) -> Result<Bytes, GatewayError> {
    let mut out = json!({
        "type":name, "sequence_number":seq, "output_index":integer(value, "output_index")?
    });
    if matches!(
        name,
        "response.output_item.added" | "response.output_item.done"
    ) {
        out["item"] = super::super::projection::project_item(&value["item"])?;
        return Ok(event(name, &out));
    }
    let item = text(value, "item_id")?;
    if !["msg_", "fc_", "rs_"]
        .iter()
        .any(|prefix| item.starts_with(prefix))
        || UpstreamId::parse(item).is_none()
    {
        return Err(invalid());
    }
    out["item_id"] = item.into();
    match name {
        "response.content_part.added" | "response.content_part.done" => {
            out["content_index"] = integer(value, "content_index")?.into();
            out["part"] = super::super::projection::project_part(&value["part"])?;
        }
        "response.output_text.delta"
        | "response.output_text.done"
        | "response.refusal.delta"
        | "response.refusal.done" => {
            let field = match name {
                "response.output_text.done" => "text",
                "response.refusal.done" => "refusal",
                _ => "delta",
            };
            out["content_index"] = integer(value, "content_index")?.into();
            out[field] = text(value, field)?.into();
            if name == "response.output_text.done" {
                out["annotations"] = json!([]);
            }
        }
        "response.function_call_arguments.delta" | "response.function_call_arguments.done" => {
            let field = if name.ends_with(".delta") {
                "delta"
            } else {
                "arguments"
            };
            out[field] = text(value, field)?.into();
        }
        "response.reasoning_summary_part.added"
        | "response.reasoning_summary_part.done"
        | "response.reasoning_summary_text.delta"
        | "response.reasoning_summary_text.done" => {
            out["summary_index"] = integer(value, "summary_index")?.into();
            if name.contains("_part.") {
                if value["part"]["type"] != "summary_text" {
                    return Err(invalid());
                }
                out["part"] = json!({"type":"summary_text","text":text(&value["part"], "text")?});
            } else {
                let field = if name.ends_with(".delta") {
                    "delta"
                } else {
                    "text"
                };
                out[field] = text(value, field)?.into();
            }
        }
        _ => return Err(invalid()),
    }
    Ok(event(name, &out))
}
