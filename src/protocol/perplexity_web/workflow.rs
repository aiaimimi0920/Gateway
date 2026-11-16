//! Observed workflow terminal only; pending patches are validated, never applied or retained.
use super::{failure, Answer};
use crate::error::GatewayError;
use serde_json::Value;

#[derive(Default)]
pub(super) struct State {
    active: bool,
}

pub(super) fn invalid() -> GatewayError {
    failure(
        "perplexity_web_invalid_workflow",
        "Perplexity Web 工作流答案或终态无效；未返回原始事件。",
    )
}

fn present(value: &Value, key: &str) -> bool {
    value.get(key).is_some_and(|value| !value.is_null())
}

impl State {
    pub(super) fn active(&self) -> bool {
        self.active
    }

    pub(super) fn recognizes(raw: &Value) -> bool {
        raw["blocks"].as_array().is_some_and(|blocks| {
            blocks.iter().any(|block| {
                block["intended_usage"] == "workflow_root"
                    || present(block, "workflow_block")
                    || block["diff_block"]["field"] == "workflow_block"
            })
        })
    }

    pub(super) fn extract(&mut self, raw: &Value) -> Result<Option<Answer>, GatewayError> {
        self.active = true;
        if ["error", "error_code", "error_message"]
            .iter()
            .any(|key| present(raw, key))
            || raw
                .get("text")
                .is_some_and(|value| !value.is_null() && value != "")
        {
            return Err(invalid());
        }
        let complete = match raw["status"].as_str() {
            Some("PENDING") => false,
            Some("COMPLETED") => true,
            _ => return Err(invalid()),
        };
        if complete
            && ["final", "text_completed", "final_sse_message"]
                .iter()
                .any(|key| raw[*key] != true)
        {
            return Err(invalid());
        }
        let blocks = raw["blocks"]
            .as_array()
            .filter(|b| b.len() <= 128)
            .ok_or_else(invalid)?;
        let mut workflow = None;
        for block in blocks {
            if !block.is_object()
                || present(block, "markdown_block")
                || present(block, "web_result_block")
            {
                return Err(invalid());
            }
            match block["intended_usage"].as_str() {
                Some("workflow_root") if workflow.is_none() => workflow = Some(block),
                Some("answer_tabs" | "pending_followups")
                    if !present(block, "workflow_block") && !present(block, "diff_block") => {}
                _ => return Err(invalid()),
            }
        }
        let block = workflow.ok_or_else(invalid)?;
        if !complete {
            validate_pending(block)?;
            return Ok(None);
        }
        if present(block, "diff_block") {
            return Err(invalid());
        }
        materialized(&block["workflow_block"]).map(Some)
    }
}

fn validate_pending(block: &Value) -> Result<(), GatewayError> {
    let diff = &block["diff_block"];
    if present(block, "workflow_block") || diff["field"] != "workflow_block" {
        return Err(invalid());
    }
    let patches = diff["patches"]
        .as_array()
        .filter(|p| !p.is_empty() && p.len() <= 4096)
        .ok_or_else(invalid)?;
    for patch in patches {
        let valid = match (patch["op"].as_str(), patch["path"].as_str()) {
            (Some("replace"), Some("")) | (Some("add"), Some("/steps/1")) => {
                patch["value"].is_object()
            }
            (
                Some("replace"),
                Some(
                    "/steps/0/title"
                    | "/steps/0/completed_at"
                    | "/status"
                    | "/started_at"
                    | "/completed_at",
                ),
            ) => patch["value"].is_string(),
            _ => false,
        };
        if !valid {
            return Err(invalid());
        }
    }
    Ok(())
}

fn materialized(workflow: &Value) -> Result<Answer, GatewayError> {
    if workflow["version"] != "1.0"
        || workflow["status"] != "WORKFLOW_COMPLETED"
        || !workflow["snapshots"].as_array().is_some_and(Vec::is_empty)
    {
        return Err(invalid());
    }
    let steps = workflow["steps"]
        .as_array()
        .filter(|steps| !steps.is_empty() && steps.len() <= 128)
        .ok_or_else(invalid)?;
    let mut answer = None;
    for step in steps {
        if step["status"] != "WORKFLOW_COMPLETED" {
            return Err(invalid());
        }
        // This captured profile has one answer item total; other step items need separate evidence.
        let items = step["items"]
            .as_array()
            .filter(|items| items.len() <= 1)
            .ok_or_else(invalid)?;
        for item in items {
            if answer.is_some()
                || item["type"] != "WORKFLOW_ITEM_TEXT"
                || item["variant"] != "answer"
            {
                return Err(invalid());
            }
            let payload = &item["payload"]["text_payload"];
            if payload["variant"] != "answer" || payload["is_streaming"] != false {
                return Err(invalid());
            }
            let text = payload["text"]
                .as_str()
                .filter(|text| !text.trim().is_empty() && text.len() <= 256 * 1024)
                .ok_or_else(invalid)?;
            let chunks = payload["chunks"]
                .as_array()
                .filter(|chunks| chunks.len() <= 4096)
                .ok_or_else(invalid)?;
            // Compare against the bounded text without allocating another concatenated answer.
            let mut remaining = text;
            for chunk in chunks {
                remaining = remaining
                    .strip_prefix(chunk.as_str().ok_or_else(invalid)?)
                    .ok_or_else(invalid)?;
            }
            if !remaining.is_empty() {
                return Err(invalid());
            }
            answer = Some(Answer {
                text: text.to_owned(),
                sources: Vec::new(),
            });
        }
    }
    answer.ok_or_else(invalid)
}
