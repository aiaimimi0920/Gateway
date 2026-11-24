//! Validate ownership before accepting assets or costs. Unknown cost never becomes zero.
use super::{reference, Kind, McpError};
use serde_json::{json, Value};
use std::collections::HashSet;

pub(crate) fn session(value: &Value) -> Result<(String, String), McpError> {
    if value["state"] != "active"
        || !value["replayed"].is_boolean()
        || !value["expires_at"].is_string()
        || !value["resource_uri"].is_string()
    {
        return Err(McpError::InvalidResponse);
    }
    Ok((
        reference(&value["session_id"], "session")?,
        reference(&value["workspace_id"], "workspace")?,
    ))
}
pub(crate) fn progress(
    value: &Value,
    session: &str,
    run: &str,
    kind: Kind,
) -> Result<Value, McpError> {
    if reference(&value["session_id"], "session")? != session
        || reference(&value["run_id"], "run")? != run
        || value["kind"] != "generate"
    {
        return Err(McpError::InvalidResponse);
    }
    let status = value["status"].as_str().ok_or(McpError::InvalidResponse)?;
    let timed_out = value["timed_out"]
        .as_bool()
        .ok_or(McpError::InvalidResponse)?;
    let outputs = value["outputs"]
        .as_array()
        .ok_or(McpError::InvalidResponse)?;
    if outputs.len() > 50 {
        return Err(McpError::Limit);
    }
    let mut ids = Vec::new();
    for output in outputs {
        // A nullable identity cannot be invented from the URL or job ID.
        if output["asset_id"].is_null() {
            continue;
        }
        let id = reference(&output["asset_id"], "artifact")?;
        if output["type"] != kind.name() || ids.contains(&id) {
            return Err(McpError::InvalidResponse);
        }
        ids.push(id);
    }
    let state = if !value["pending"].is_null() {
        "needs_input"
    } else if !value["settled_with"].is_null() {
        "superseded"
    } else if timed_out {
        "running"
    } else {
        match status {
            "completed" | "failed" | "cancelled" | "running" | "needs_input" => status,
            _ => "unconfirmed",
        }
    };
    Ok(json!({"state":state,"assetIds":ids,"timedOut":timed_out}))
}
pub(crate) fn assets(value: &Value, ids: &[String], kind: Kind) -> Result<Vec<Value>, McpError> {
    let rows = value["assets"]
        .as_array()
        .ok_or(McpError::InvalidResponse)?;
    if rows.len() != ids.len() {
        return Err(McpError::InvalidResponse);
    }
    let mut seen = HashSet::new();
    rows.iter()
        .map(|row| {
            let id = reference(&row["asset_id"], "artifact")?;
            if !ids.contains(&id)
                || !seen.insert(id.clone())
                || row["type"] != kind.name()
                || !row["has_image"].is_boolean()
                || !matches!(
                    row["state"].as_str(),
                    Some("pending" | "processing" | "ready" | "failed")
                )
            {
                return Err(McpError::InvalidResponse);
            }
            let url = if row["state"] == "ready" {
                row["url"]
                    .as_str()
                    .map(|raw| {
                        crate::http_client::public_asset_url::checked(raw)
                            .map(|url| url.to_string())
                            .map_err(|_| McpError::InvalidResponse)
                    })
                    .transpose()?
            } else {
                None
            };
            Ok(json!({"id":id,"kind":kind.name(),"state":row["state"],"url":url}))
        })
        .collect()
}
pub(crate) fn usage(value: &Value, session: &str) -> Result<Value, McpError> {
    let value = &value["usage"];
    if reference(&value["session_id"], "session")? != session {
        return Err(McpError::InvalidResponse);
    }
    let credits = value["total_credits"]
        .as_u64()
        .ok_or(McpError::InvalidResponse)?;
    let unresolved = value["unresolved_event_count"]
        .as_u64()
        .ok_or(McpError::InvalidResponse)?;
    let mut estimated = false;
    if let Some(jobs) = value.get("jobs") {
        let jobs = jobs.as_array().ok_or(McpError::InvalidResponse)?;
        if jobs.len() > 256 {
            return Err(McpError::Limit);
        }
        for job in jobs {
            if !job["job_id"].is_string() || job["credits"].as_u64().is_none() {
                return Err(McpError::InvalidResponse);
            }
            estimated |= job["credits_estimated"]
                .as_bool()
                .ok_or(McpError::InvalidResponse)?;
        }
    }
    Ok(
        json!({"credits":credits,"settled":unresolved == 0 && !estimated,
        "unresolvedEvents":unresolved,"estimated":estimated,"scope":"session"}),
    )
}
pub(crate) fn cancellation(
    value: &Value,
    session: &str,
    run: Option<&str>,
) -> Result<String, McpError> {
    if let Some(run) = run {
        if reference(&value["run_id"], "run")? != run || value["status"] != "cancelled" {
            return Err(McpError::InvalidResponse);
        }
        if !value["session_id"].is_null() && reference(&value["session_id"], "session")? != session
        {
            return Err(McpError::InvalidResponse);
        }
        Ok("cancelled".into())
    } else {
        if reference(&value["session_id"], "session")? != session || !value["run_id"].is_null() {
            return Err(McpError::InvalidResponse);
        }
        match value["status"].as_str() {
            Some("ended" | "cancelled" | "completed" | "closed") => Ok("ended".into()),
            _ => Err(McpError::InvalidResponse),
        }
    }
}
