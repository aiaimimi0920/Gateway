//! Public JSON is built from the durable receipt and typed wire fields, not copied from SAP.
use super::{dispatch::Observation, RoutePin};
use crate::error::GatewayError;
use serde_json::{json, Value};

impl Observation {
    pub(super) fn public_json(&self, pin: &RoutePin) -> Result<Value, GatewayError> {
        let row = &self.receipt;
        if row.phase == "deleted" {
            if self.body["object"] != "response.deleted"
                || self.body["deleted"] != true
                || self.body["id"].as_str() != row.upstream_id.as_deref()
            {
                return Err(conflict());
            }
            return Ok(json!({"id":row.id,"object":"response.deleted","deleted":true}));
        }
        let upstream = super::dispatch::live_id(row).ok_or_else(conflict)?;
        let (_, state) = super::super::observe(&self.body, Some(&upstream))?;
        use super::super::State;
        let (phase, usage) = match state {
            State::Pending => ("pending", None),
            State::Completed(usage) => ("completed", usage),
            State::Failed(usage) => ("failed", usage),
        };
        let durable_usage = row
            .input_tokens
            .zip(row.output_tokens)
            .and_then(|(input, output)| {
                Some((u64::try_from(input).ok()?, u64::try_from(output).ok()?))
            });
        if row.phase != phase
            || usage
                .as_ref()
                .map(|u| (u.prompt_tokens, u.completion_tokens))
                != durable_usage
        {
            return Err(conflict());
        }
        project_response(row, pin, &self.body, usage)
    }
}

pub(super) fn historical_pending(
    row: &super::Receipt,
    pin: &RoutePin,
    body: &Value,
) -> Result<Value, GatewayError> {
    let upstream = super::dispatch::live_id(row).ok_or_else(conflict)?;
    if !matches!(
        super::super::observe(body, Some(&upstream))?.1,
        super::super::State::Pending
    ) {
        return Err(conflict());
    }
    project_response(row, pin, body, None)
}

fn project_response(
    row: &super::Receipt,
    pin: &RoutePin,
    body: &Value,
    usage: Option<crate::protocol::canonical::TokenUsage>,
) -> Result<Value, GatewayError> {
    let output = body["output"]
        .as_array()
        .ok_or_else(conflict)?
        .iter()
        .map(project_item)
        .collect::<Result<Vec<_>, _>>()?;
    let status = body["status"].as_str().ok_or_else(conflict)?;
    let mut public = json!({
        "id": row.id, "object": "response", "model": pin.public_model,
        "status": status, "store": true, "output": output,
        "usage": usage.map(|u| json!({
            "input_tokens":u.prompt_tokens,"output_tokens":u.completion_tokens,
            "total_tokens":u.total_tokens
        })),
        "error": null,
    });
    if status == "failed" {
        public["error"] =
            json!({"code":"sap_aicore_response_failed","message":"Stored response failed."});
    }
    if let Some(at) = body["created_at"].as_u64() {
        public["created_at"] = at.into();
    }
    if status == "incomplete" {
        let reason = match body["incomplete_details"]["reason"].as_str() {
            Some("max_output_tokens") => "max_output_tokens",
            Some("content_filter") => "content_filter",
            _ => "other",
        };
        public["incomplete_details"] = json!({"reason":reason});
    }
    Ok(public)
}

pub(super) fn project_item(item: &Value) -> Result<Value, GatewayError> {
    let mut output = match item["type"].as_str() {
        Some("message") if item["role"] == "assistant" => {
            let parts = item["content"]
                .as_array()
                .ok_or_else(conflict)?
                .iter()
                .map(project_part)
                .collect::<Result<Vec<_>, _>>()?;
            json!({"type":"message","role":"assistant","content":parts})
        }
        Some("function_call") => json!({
            "type":"function_call","call_id": field(item,"call_id")?,
            "name":field(item,"name")?,"arguments":field(item,"arguments")?
        }),
        Some("reasoning") => {
            let summary = item["summary"]
                .as_array()
                .ok_or_else(conflict)?
                .iter()
                .map(|part| {
                    if part["type"] != "summary_text" {
                        return Err(conflict());
                    }
                    Ok(json!({"type":"summary_text","text":field(part,"text")?}))
                })
                .collect::<Result<Vec<_>, _>>()?;
            json!({"type":"reasoning","summary":summary})
        }
        _ => return Err(conflict()),
    };
    if let Some(id) = item["id"].as_str() {
        // Response IDs are never item IDs. Retain only the admitted item namespaces.
        let prefix = match item["type"].as_str() {
            Some("message") => "msg_",
            Some("function_call") => "fc_",
            _ => "rs_",
        };
        if id.starts_with(prefix) && super::UpstreamId::parse(id).is_some() {
            output["id"] = id.into();
        }
    }
    if let Some(status @ ("in_progress" | "completed" | "incomplete")) = item["status"].as_str() {
        output["status"] = status.into();
    }
    Ok(output)
}

pub(super) fn project_part(part: &Value) -> Result<Value, GatewayError> {
    match part["type"].as_str() {
        Some("output_text") => {
            Ok(json!({"type":"output_text","text":field(part,"text")?,"annotations":[]}))
        }
        Some("refusal") => Ok(json!({"type":"refusal","refusal":field(part,"refusal")?})),
        _ => Err(conflict()),
    }
}

fn field<'a>(item: &'a Value, key: &str) -> Result<&'a str, GatewayError> {
    item[key].as_str().ok_or_else(conflict)
}

fn conflict() -> GatewayError {
    GatewayError::conflict("Stored SAP response does not match its durable receipt.")
        .with_code("sap_aicore_response_observation_conflict")
}
