//! 有限 markdown profile；公开 wire 依据见 provider-perplexity-schematized-contract.md。
//! 不实现通用 JSON Patch，不将 EOF、final 标志或其他 track 当作答案终态。
use super::{failure, snapshot, Answer};
use crate::error::GatewayError;
use serde_json::{json, Value};

const LIMIT: usize = 256 * 1024;
const CHUNKS: usize = 4096;
const USAGE: &str = "ask_text_0_markdown";

#[derive(Default)]
pub(super) struct State {
    active: bool,
    chunks: Option<Vec<String>>,
    bytes: usize,
}

pub(super) fn invalid() -> GatewayError {
    failure(
        "perplexity_web_invalid_schematized",
        "Perplexity Web 增量响应格式或终态无效；未返回原始事件。",
    )
}

impl State {
    pub(super) fn active(&self) -> bool {
        self.active
    }

    pub(super) fn recognizes(raw: &Value) -> bool {
        raw["blocks"].as_array().is_some_and(|blocks| {
            blocks.iter().any(|block| {
                block["intended_usage"] == USAGE
                    || block["intended_usage"] == "workflow_root"
                    || block
                        .get("workflow_block")
                        .is_some_and(|value| !value.is_null())
                    || block["diff_block"]["field"] == "workflow_block"
            })
        })
    }

    // The caller selects this owner once; auto/legacy cannot silently replace its state.
    pub(super) fn extract(&mut self, raw: &Value) -> Result<(Option<Answer>, bool), GatewayError> {
        self.active = true;
        if !raw.is_object()
            || ["error", "error_code", "error_message"]
                .iter()
                .any(|key| raw.get(*key).is_some_and(|value| !value.is_null()))
        {
            return Err(invalid());
        }
        let complete = match raw["status"].as_str() {
            Some("PENDING") => false,
            Some("COMPLETED") => true,
            _ => return Err(invalid()),
        };
        let blocks = raw["blocks"]
            .as_array()
            .filter(|blocks| blocks.len() <= 128)
            .ok_or_else(invalid)?;
        let mut normalized = Vec::with_capacity(blocks.len());
        let mut seen = false;
        for block in blocks {
            if block
                .get("workflow_block")
                .is_some_and(|value| !value.is_null())
            {
                return Err(invalid());
            }
            if block["intended_usage"] != USAGE {
                // Other answer tracks and workflow cannot be joined by length or guessed identity.
                if block["intended_usage"]
                    .as_str()
                    .is_some_and(|usage| usage.starts_with("ask_text") || usage == "workflow_root")
                {
                    return Err(invalid());
                }
                normalized.push(block.clone());
                continue;
            }
            if seen {
                return Err(invalid());
            }
            seen = true;
            if let Some(diff) = block.get("diff_block").filter(|value| !value.is_null()) {
                if complete
                    || block
                        .get("markdown_block")
                        .is_some_and(|value| !value.is_null())
                    || diff["field"] != "markdown_block"
                {
                    return Err(invalid());
                }
                let patches = diff["patches"]
                    .as_array()
                    .filter(|patches| !patches.is_empty() && patches.len() <= CHUNKS)
                    .ok_or_else(invalid)?;
                for patch in patches {
                    self.apply(patch)?;
                }
            } else if complete && block["markdown_block"].get("chunks").is_none() {
                // Answer-only is a materialized terminal, never a synthetic seed for chunk appends.
                let text = block["markdown_block"]["answer"]
                    .as_str()
                    .filter(|text| text.len() <= LIMIT)
                    .ok_or_else(invalid)?;
                self.bytes = text.len();
                self.chunks = Some(vec![text.to_owned()]);
            } else {
                self.replace(&block["markdown_block"])?;
            }
            let text = self.chunks.as_ref().ok_or_else(invalid)?.concat();
            normalized.push(json!({"intended_usage":"ask_text","markdown_block":{"answer":text}}));
        }
        if complete && !seen {
            return Err(invalid());
        }
        // Only this event's sources survive. Native access tokens and prior sources never carry over.
        let mut event = raw.clone();
        event["blocks"] = Value::Array(normalized);
        if event
            .get("text")
            .is_some_and(|value| !value.is_null() && value != "")
        {
            return Err(invalid());
        }
        let answer = snapshot::extract(&event)?;
        if complete
            && answer
                .as_ref()
                .is_none_or(|answer| answer.text.trim().is_empty())
        {
            return Err(invalid());
        }
        Ok((answer, complete))
    }

    fn replace(&mut self, value: &Value) -> Result<(), GatewayError> {
        let values = value["chunks"]
            .as_array()
            .filter(|values| values.len() <= CHUNKS)
            .ok_or_else(invalid)?;
        let mut bytes = 0usize;
        let mut chunks = Vec::with_capacity(values.len());
        for value in values {
            let text = value.as_str().ok_or_else(invalid)?;
            bytes = bytes
                .checked_add(text.len())
                .filter(|bytes| *bytes <= LIMIT)
                .ok_or_else(invalid)?;
            chunks.push(text.to_owned());
        }
        if let Some(answer) = value.get("answer").filter(|value| !value.is_null()) {
            if answer.as_str() != Some(chunks.concat().as_str()) {
                return Err(invalid());
            }
        }
        self.chunks = Some(chunks);
        self.bytes = bytes;
        Ok(())
    }

    fn apply(&mut self, patch: &Value) -> Result<(), GatewayError> {
        match (patch["op"].as_str(), patch["path"].as_str()) {
            (Some("replace"), Some("")) => self.replace(&patch["value"]),
            (Some("add"), Some(path)) => {
                let chunks = self.chunks.as_mut().ok_or_else(invalid)?;
                // Only observed contiguous appends are supported; no sparse allocation or overwrite.
                if chunks.len() >= CHUNKS || path != format!("/chunks/{}", chunks.len()) {
                    return Err(invalid());
                }
                let text = patch["value"].as_str().ok_or_else(invalid)?;
                self.bytes = self
                    .bytes
                    .checked_add(text.len())
                    .filter(|bytes| *bytes <= LIMIT)
                    .ok_or_else(invalid)?;
                chunks.push(text.to_owned());
                Ok(())
            }
            _ => Err(invalid()),
        }
    }
}
