//! Bounded envelope state: only one matched session and turn can yield a successful answer.
use super::failure;
use crate::error::{ErrorKind, GatewayError};
use serde_json::Value;

pub(super) const FRAME_LIMIT: usize = 1024 * 1024;
pub(super) const BODY_LIMIT: usize = 8 * 1024 * 1024;
pub(super) const TEXT_LIMIT: usize = 2 * 1024 * 1024;
pub(super) enum Action {
    None,
    Turn(String),
    Text(String),
    Done,
}
pub(super) struct Machine {
    initial: String,
    session: Option<String>,
    response: Option<String>,
    created: bool,
    attached: bool,
    sent: bool,
    done: bool,
    frames: usize,
    bytes: usize,
    text_bytes: usize,
}
impl Machine {
    pub fn new(initial: String) -> Self {
        Self {
            initial,
            session: None,
            response: None,
            created: false,
            attached: false,
            sent: false,
            done: false,
            frames: 0,
            bytes: 0,
            text_bytes: 0,
        }
    }
    pub fn push(&mut self, raw: &str) -> Result<Action, GatewayError> {
        self.frames += 1;
        self.bytes = self.bytes.saturating_add(raw.len());
        if self.done || self.frames > 16384 || raw.len() > FRAME_LIMIT || self.bytes > BODY_LIMIT {
            return Err(failure("grok_web_response_limit", ErrorKind::ServerError));
        }
        let envelope: Value = serde_json::from_str(raw).map_err(|_| protocol())?;
        let event = envelope
            .get("event")
            .filter(|v| v.is_object())
            .ok_or_else(protocol)?;
        let kind = event["type"].as_str().ok_or_else(protocol)?;
        if let Some(id) = envelope.get("session_id") {
            let id = id
                .as_str()
                .filter(|id| !id.is_empty())
                .ok_or_else(protocol)?;
            self.bind(id)?;
        }
        match kind {
            "session.created" | "conversation.attached" => {
                if event.get("client_event_id").is_some_and(|id| {
                    !matches!(id.as_str(), Some("") | None) && id.as_str() != Some(&self.initial)
                }) {
                    return Err(protocol());
                }
                if kind == "session.created" {
                    let id = envelope["session_id"].as_str().ok_or_else(protocol)?;
                    self.bind(id)?;
                    self.created = true;
                } else {
                    self.bind(event["conversation"]["id"].as_str().ok_or_else(protocol)?)?;
                    self.attached = true;
                }
                if self.created && self.attached && !self.sent {
                    self.sent = true;
                    return Ok(Action::Turn(self.session.clone().ok_or_else(protocol)?));
                }
            }
            "error" | "session.ended" => {
                return Err(failure("grok_web_upstream_failed", ErrorKind::ServerError))
            }
            kind if kind.starts_with("response.") => {
                if !self.sent {
                    return Err(protocol());
                }
                for id in [
                    event.get("response_id"),
                    event.get("response").and_then(|v| v.get("id")),
                ]
                .into_iter()
                .flatten()
                {
                    let id = id
                        .as_str()
                        .filter(|v| !v.is_empty() && v.len() <= 256)
                        .ok_or_else(protocol)?;
                    if self.response.as_deref().is_some_and(|v| v != id) {
                        return Err(protocol());
                    }
                    if self.response.is_none() {
                        self.response = Some(id.into());
                    }
                }
                match kind {
                    "response.chunk" => {
                        let chunk = event
                            .get("chunk")
                            .filter(|v| v.is_object())
                            .ok_or_else(protocol)?;
                        if ["tool_usage_card", "tool_result", "render_citation"]
                            .into_iter()
                            .any(|key| chunk.get(key).is_some())
                        {
                            return Err(failure(
                                "grok_web_unsupported_output",
                                ErrorKind::ServerError,
                            ));
                        }
                        if let Some(text) = chunk.get("text") {
                            let delta = text["text"].as_str().ok_or_else(protocol)?;
                            return match text["channel"].as_str().unwrap_or("") {
                                "" | "CHANNEL_ASSISTANT_RESPONSE" => self.text(delta),
                                "ANALYSIS" | "REASONING" | "CHANNEL_ANALYSIS"
                                | "CHANNEL_REASONING" => Ok(Action::None),
                                _ => Err(failure(
                                    "grok_web_unsupported_output",
                                    ErrorKind::ServerError,
                                )),
                            };
                        }
                    }
                    "response.output_text.delta" => {
                        return self.text(event["delta"].as_str().ok_or_else(protocol)?)
                    }
                    "response.output_text.done" => {
                        let text = event["text"].as_str().ok_or_else(protocol)?;
                        if self.text_bytes == 0 {
                            return self.text(text);
                        }
                    }
                    "response.done" => {
                        if event["response"]["status"] != "completed"
                            || self.text_bytes == 0
                            || event["response"]["id"]
                                .as_str()
                                .is_none_or(|s| s.is_empty() || s.len() > 256)
                        {
                            return Err(failure("grok_web_incomplete", ErrorKind::ServerError));
                        }
                        self.done = true;
                        return Ok(Action::Done);
                    }
                    "response.grok.output" => {
                        if event["output"].get("stream_error").is_some() {
                            return Err(failure(
                                "grok_web_upstream_failed",
                                ErrorKind::ServerError,
                            ));
                        }
                        if event["output"].get("card_attachment").is_some() {
                            return Err(failure(
                                "grok_web_unsupported_output",
                                ErrorKind::ServerError,
                            ));
                        }
                    }
                    "response.search.result" => {
                        return Err(failure(
                            "grok_web_unsupported_output",
                            ErrorKind::ServerError,
                        ))
                    }
                    // New response kinds are not assumed to be harmless lifecycle events.
                    _ => return Err(protocol()),
                }
            }
            "ping" | "pong" => {}
            _ => return Err(protocol()),
        }
        Ok(Action::None)
    }
    fn bind(&mut self, id: &str) -> Result<(), GatewayError> {
        if id.is_empty() || id.len() > 256 || self.session.as_deref().is_some_and(|v| v != id) {
            return Err(protocol());
        }
        if self.session.is_none() {
            self.session = Some(id.into());
        }
        Ok(())
    }
    fn text(&mut self, text: &str) -> Result<Action, GatewayError> {
        self.text_bytes = self.text_bytes.saturating_add(text.len());
        if self.text_bytes > TEXT_LIMIT {
            return Err(failure("grok_web_response_limit", ErrorKind::ServerError));
        }
        Ok(if text.is_empty() {
            Action::None
        } else {
            Action::Text(text.into())
        })
    }
}
pub(super) fn protocol() -> GatewayError {
    failure("grok_web_protocol_invalid", ErrorKind::ServerError)
}
