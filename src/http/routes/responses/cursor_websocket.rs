//! Cursor-only WS admission and delivery ownership. No stored-response emulation.
//! Every socket has an unforgeable binding and owns only its currently parked epoch.
use crate::{
    pipeline::PipelineContext, protocol::cursor_remote::runtime::Publication,
    routing::config::cursor,
};
use serde_json::Value;
use tokio::sync::oneshot;

pub(super) fn project_body(body: &mut Value) {
    if cursor::private_request(body["model"].as_str()) {
        // The WS owner has already checked response.create. Preserve the original
        // wire separately, but do not send this transport envelope to Cursor.
        body.as_object_mut().unwrap().remove("type");
        body["stream"] = Value::Bool(true);
    }
}

pub(super) struct Connection {
    id: uuid::Uuid,
    parked: Option<(String, Publication)>,
}
impl Connection {
    pub(super) fn new() -> Self {
        Self {
            id: uuid::Uuid::new_v4(),
            parked: None,
        }
    }
    pub(super) fn begin(&self, ctx: &mut PipelineContext) -> Option<Turn> {
        if !cursor::private_request(ctx.canonical_req.requested_model.as_deref()) {
            return None;
        }
        let (sender, receiver) = oneshot::channel();
        ctx.cursor_remote_connection = Some(self.id);
        ctx.cursor_remote_continuation = ctx.canonical_req.raw_body["previous_response_id"]
            .as_str()
            .and_then(|id| self.parked.as_ref().filter(|(previous, _)| previous == id))
            .and_then(|(id, guard)| guard.continuation(id));
        ctx.cursor_tool_publication = Some(sender);
        Some(Turn {
            receiver,
            terminal: None,
        })
    }
    pub(super) fn finish(&mut self, ready: Ready) -> Result<(), ()> {
        if let Some(guard) = &ready.publication {
            if !guard.publish_on_connection() {
                return Err(());
            }
        }
        // The former epoch is stale after a valid continuation; its Drop cannot
        // revoke this new pause. On socket loss, the current epoch is cancelled.
        self.parked = ready.publication.map(|guard| (ready.response_id, guard));
        Ok(())
    }
}

pub(super) struct Turn {
    // Drop also cancels a guard still queued here when a write/stream fails.
    receiver: oneshot::Receiver<Publication>,
    terminal: Option<(String, bool, String)>,
}
pub(super) struct Ready {
    pub(super) data: String,
    publication: Option<Publication>,
    response_id: String,
}
impl Turn {
    pub(super) fn terminal_received(&self) -> bool {
        self.terminal.is_some()
    }
    /// Validate before writing. Hold only the terminal frame until strict EOF and
    /// publication checks pass; a malformed suffix must not leak a success event.
    pub(super) fn accept(&mut self, data: &str) -> Result<bool, ()> {
        if self.terminal.is_some() {
            return Err(());
        }
        let event: Value = serde_json::from_str(data).map_err(|_| ())?;
        match event["type"].as_str().ok_or(())? {
            "response.completed" if event["response"]["status"] == "completed" => {
                let output = event["response"]["output"].as_array().ok_or(())?;
                let response_id = event["response"]["id"]
                    .as_str()
                    .filter(|id| id.starts_with("resp_") && id.len() <= 256)
                    .ok_or(())?;
                let has_tool = output.iter().any(|item| {
                    matches!(
                        item["type"].as_str(),
                        Some("function_call" | "custom_tool_call")
                    )
                });
                self.terminal = Some((data.to_owned(), has_tool, response_id.into()));
                return Ok(false);
            }
            "error" | "response.failed" | "response.incomplete" | "response.completed" => {
                return Err(());
            }
            _ => {}
        }
        Ok(true)
    }
    /// The caller must establish strict EOF before requesting the final frame.
    pub(super) fn prepare(mut self) -> Result<Ready, ()> {
        let (data, has_tool, response_id) = self.terminal.ok_or(())?;
        let publication = self.receiver.try_recv().ok();
        if has_tool != publication.is_some()
            || publication
                .as_ref()
                .is_some_and(|guard| !guard.can_publish())
        {
            return Err(());
        }
        Ok(Ready {
            data,
            publication,
            response_id,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn cursor_websocket_projection_preserves_other_provider_bodies() {
        let mut body = json!({"type":"response.create","model":"cursor-personal","input":"hello"});
        project_body(&mut body);
        assert!(body.get("type").is_none());
        assert_eq!(body["stream"], true);
        let original = json!({"type":"response.create","model":"other","future_field":true});
        let mut other = original.clone();
        project_body(&mut other);
        assert_eq!(other, original);
    }

    #[test]
    fn cursor_websocket_publication_requires_one_successful_terminal() {
        for data in [
            r#"{"type":"response.failed"}"#,
            r#"{"type":"response.incomplete"}"#,
            r#"{"type":"response.completed","response":{"status":"failed","output":[]}}"#,
            r#"{"type":"response.completed","response":{"status":"completed"}}"#,
            "[DONE]",
        ] {
            let (_, receiver) = oneshot::channel();
            let mut turn = Turn {
                receiver,
                terminal: None,
            };
            assert!(turn.accept(data).is_err());
        }
        let mut owner = Connection::new();
        let (_, receiver) = oneshot::channel();
        assert!(Turn {
            receiver,
            terminal: None
        }
        .prepare()
        .is_err());
        let (_, receiver) = oneshot::channel();
        let mut turn = Turn {
            receiver,
            terminal: None,
        };
        let completed = r#"{"type":"response.completed","response":{"id":"resp_fixture","status":"completed","output":[]}}"#;
        assert!(!turn.accept(completed).unwrap()); // terminal withheld, not sent
        let ready = turn.prepare().unwrap();
        assert_eq!(ready.data, completed);
        owner.finish(ready).unwrap();
        let (_, receiver) = oneshot::channel();
        assert!(Turn {
            receiver,
            terminal: Some((completed.into(), true, "resp_fixture".into()))
        }
        .prepare()
        .is_err());
        for suffix in [
            completed,
            "not JSON",
            r#"{"type":"response.output_text.delta"}"#,
        ] {
            let (_, receiver) = oneshot::channel();
            let mut turn = Turn {
                receiver,
                terminal: None,
            };
            assert!(!turn.accept(completed).unwrap());
            assert!(turn.accept(suffix).is_err()); // no terminal may be forwarded
        }
    }
}
