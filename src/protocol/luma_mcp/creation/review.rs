//! Keep the supplier's review contract opaque; never invent a verdict or execute a client tool.
use super::{reference, McpError};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Review {
    pub fingerprint: String,
    pub invocation_id: Option<String>,
    pub kind: String,
    pub content: Value,
    pub answer_format: Option<Value>,
    pub show: Vec<String>,
    pub assets: Vec<Value>,
}
impl Review {
    pub(crate) fn answerable(&self) -> bool {
        (self.kind == "review" && self.answer_format.is_some()
            || super::questions::supported(&self.kind))
            && self.invocation_id.is_some()
            && self.show.iter().all(|id| {
                self.assets.iter().any(|asset| {
                    asset["id"] == *id && asset["state"] == "ready" && asset["url"].is_string()
                })
            })
    }
    pub(crate) fn accepts_answer(&self, value: &Value) -> bool {
        self.kind != "select" || super::questions::accepts_selection(self, value)
    }
}
pub(crate) fn digest(value: &Value) -> String {
    format!("{:x}", Sha256::digest(value.to_string().as_bytes()))
}
pub(crate) fn review(value: &Value, run: &str) -> Result<Option<Review>, McpError> {
    if value.is_null() {
        return Ok(None);
    }
    if !value.is_object() || value.to_string().len() > 24 * 1024 {
        return Err(McpError::Limit);
    }
    if !value["run_id"].is_null() && reference(&value["run_id"], "run")? != run {
        return Err(McpError::InvalidResponse);
    }
    let interaction = &value["interaction"];
    let is_gate = value["type"] == "agent.gate" && value["truncated"] != true;
    let kind = if is_gate {
        interaction["kind"].as_str().unwrap_or("unsupported")
    } else {
        "unsupported"
    };
    let mut result = Review {
        fingerprint: digest(value),
        invocation_id: None,
        kind: kind.into(),
        content: json!({}),
        answer_format: None,
        show: vec![],
        assets: vec![],
    };
    // Other questions remain visible, but arbitrary client tool parameters are not exposed or run.
    for key in ["message", "prompt"] {
        if is_gate && interaction[key].is_string() {
            result.content[key] = interaction[key].clone();
        }
    }
    if kind != "review" {
        super::questions::populate(&mut result, interaction)?;
        return Ok(Some(result));
    }
    let invocation = interaction["invocation_id"]
        .as_str()
        .ok_or(McpError::InvalidResponse)?;
    if invocation.is_empty()
        || invocation.chars().count() > 128
        || invocation.chars().any(char::is_control)
        || !interaction["params"].is_object()
    {
        return Err(McpError::InvalidResponse);
    }
    result.invocation_id = Some(invocation.into());
    result.content["params"] = interaction["params"].clone();
    let format = &interaction["answer_format"];
    if !format.is_null() {
        let targets = format["revision_targets"]
            .as_array()
            .ok_or(McpError::InvalidResponse)?;
        if !format["verdict_schema"].is_object()
            || targets.len() > 128
            || targets.iter().any(|v| !v.is_string())
        {
            return Err(McpError::InvalidResponse);
        }
        result.answer_format = Some(format.clone());
    }
    for key in ["show", "text"] {
        if interaction[key].is_null() {
            continue;
        }
        let rows = interaction[key]
            .as_array()
            .ok_or(McpError::InvalidResponse)?;
        if rows.len() > 50 {
            return Err(McpError::Limit);
        }
        for row in rows {
            if !row["target"].is_string() {
                return Err(McpError::InvalidResponse);
            }
            if key == "show" {
                let id = reference(&row["asset_id"], "artifact")?;
                if !result.show.contains(&id) {
                    result.show.push(id);
                }
            } else if !row["label"].is_string() || !row["text"].is_string() {
                return Err(McpError::InvalidResponse);
            }
        }
        result.content[key] = interaction[key].clone();
    }
    Ok(Some(result))
}

// Session pages and run polls can announce the same action in different shapes.
// Never choose between multiple actions for the saved run or adopt a foreign run ID.
pub(crate) fn select_review(
    pending: &Value,
    page: Option<&Value>,
    run: &str,
) -> Result<Option<Review>, McpError> {
    let direct = review(pending, run)?;
    let Some(page) =
        page.filter(|page| page["status"] == "requires_action" && page["has_more"] != true)
    else {
        return Ok(direct);
    };
    let Some(waiting) = page["waiting"].as_array() else {
        return Ok(direct);
    };
    let mut matching = waiting
        .iter()
        .filter(|item| reference(&item["run_id"], "run").is_ok_and(|id| id == run));
    let Some(item) = matching.next() else {
        return Ok(direct);
    };
    if matching.next().is_some()
        || item["kind"] != "gate"
        || direct
            .as_ref()
            .is_some_and(|review| review.invocation_id.as_deref() != item["invocation_id"].as_str())
    {
        return Ok(None);
    }
    if let Some(mut question) = direct
        .clone()
        .filter(|r| super::questions::supported(&r.kind))
    {
        // The session summary cannot supply the missing typed question. Only accept
        // a matching full run question; the supplier chooses its response wire type.
        if !item["answer_format"].is_null()
            || (!pending["answer_type"].is_null()
                && !item["answer_type"].is_null()
                && pending["answer_type"] != item["answer_type"])
        {
            return Ok(None);
        }
        question.fingerprint = digest(&json!([item, pending]));
        return Ok(Some(question));
    }
    if item["answer_type"] != "user.gate_verdict" || item["answer_format"].is_null() {
        return Ok(None);
    }
    let mut normalized = json!({"type":"agent.gate","run_id":run,"interaction":{
        "kind":"review","invocation_id":item["invocation_id"],"answer_format":item["answer_format"],
        "show":item["show"],"text":item["text"],"params":{}
    }});
    if let Some(message) = item["message"].as_str() {
        normalized["interaction"]["message"] = json!(message);
    }
    let Some(mut selected) = review(&normalized, run)? else {
        return Ok(None);
    };
    if let Some(direct) = direct {
        // Preserve the original prompt and required media. A second source may
        // omit optional context, but must not contradict the question it names.
        if selected.answer_format != direct.answer_format
            || ["show", "text"]
                .iter()
                .any(|key| !item[*key].is_null() && selected.content[*key] != direct.content[*key])
        {
            return Ok(None);
        }
        selected = direct;
    }
    selected.fingerprint = digest(&json!([item, pending]));
    Ok(Some(selected))
}

pub(crate) fn review_assets(value: &Value, ids: &[String]) -> Result<Vec<Value>, McpError> {
    let rows = value["assets"]
        .as_array()
        .ok_or(McpError::InvalidResponse)?;
    if rows.len() != ids.len() {
        return Err(McpError::InvalidResponse);
    }
    let mut assets = Vec::new();
    for row in rows {
        let id = reference(&row["asset_id"], "artifact")?;
        if !ids.contains(&id) || assets.iter().any(|v: &Value| v["id"] == id) {
            return Err(McpError::InvalidResponse);
        }
        let kind = match row["type"].as_str() {
            Some("image") => super::Kind::Image,
            Some("video") => super::Kind::Video,
            _ => return Err(McpError::Unsupported),
        };
        assets.extend(super::assets(&json!({"assets":[row]}), &[id], kind)?);
    }
    Ok(assets)
}

pub(crate) fn delivered(value: &Value, session: &str, run: &str) -> Result<u64, McpError> {
    if value["status"] != "delivered"
        || reference(&value["session_id"], "session")? != session
        || reference(&value["run_id"], "run")? != run
    {
        return Err(McpError::InvalidResponse);
    }
    // A nullable receipt seq requires reading from the beginning; never guess a later cursor.
    if value["seq"].is_null() {
        Ok(0)
    } else {
        value["seq"].as_u64().ok_or(McpError::InvalidResponse)
    }
}
pub(crate) fn session_update(value: &Value, session: &str, after: u64) -> Result<u64, McpError> {
    if value.to_string().len() > 64 * 1024 {
        return Err(McpError::Limit);
    }
    let status = value["status"].as_str().ok_or(McpError::InvalidResponse)?;
    let next = value["next_after_seq"]
        .as_u64()
        .ok_or(McpError::InvalidResponse)?;
    let events = value["events"]
        .as_array()
        .ok_or(McpError::InvalidResponse)?;
    if reference(&value["session_id"], "session")? != session
        || !matches!(
            status,
            "working" | "your_turn" | "requires_action" | "ended"
        )
        || !value["timed_out"].is_boolean()
        || events.len() > 100
        || events.iter().any(|event| !event.is_object())
        || next < after
        || (status == "working" && (next != after || !events.is_empty()))
        || (!value["has_more"].is_null() && !value["has_more"].is_boolean())
        || (value["has_more"] == true && next == after)
        || value.get("waiting").is_some_and(|waiting| {
            !waiting.as_array().is_some_and(|entries| {
                entries.len() <= 100
                    && entries.iter().all(Value::is_object)
                    && (entries.is_empty() || status == "requires_action")
            })
        })
    {
        return Err(McpError::InvalidResponse);
    }
    Ok(next)
}
