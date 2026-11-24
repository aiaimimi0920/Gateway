//! Project typed user questions without inventing the opaque response schema or invoking client tools.
use super::{reference, review::Review, McpError};
use serde_json::{json, Value};

pub(super) fn supported(kind: &str) -> bool {
    matches!(kind, "approve" | "confirm" | "text_input" | "select")
}

pub(super) fn accepts_selection(review: &Review, value: &Value) -> bool {
    let Some(ids) = value.as_array() else {
        return false;
    };
    let (Some(min), Some(max), Some(options)) = (
        review.content["min"].as_u64(),
        review.content["max"].as_u64(),
        review.content["options"].as_array(),
    ) else {
        return false;
    };
    if (ids.len() as u64) < min || (ids.len() as u64) > max {
        return false;
    }
    let mut seen = std::collections::HashSet::new();
    ids.iter().all(|id| {
        reference(id, "artifact").is_ok_and(|id| {
            seen.insert(id.clone())
                && options.iter().any(|option| {
                    reference(&option["asset_id"], "artifact").is_ok_and(|offered| offered == id)
                })
        })
    })
}

pub(super) fn populate(result: &mut Review, interaction: &Value) -> Result<(), McpError> {
    if !supported(&result.kind) {
        return Ok(());
    }
    let invocation = interaction["invocation_id"]
        .as_str()
        .ok_or(McpError::InvalidResponse)?;
    if invocation.is_empty()
        || invocation.chars().count() > 128
        || invocation.chars().any(char::is_control)
    {
        return Err(McpError::InvalidResponse);
    }
    match result.kind.as_str() {
        "approve" | "confirm" => {
            if !interaction["message"].is_string() {
                return Err(McpError::InvalidResponse);
            }
            if result.kind == "confirm" {
                if !interaction["destructive"].is_boolean() {
                    return Err(McpError::InvalidResponse);
                }
                result.content["destructive"] = interaction["destructive"].clone();
            }
        }
        "text_input" => {
            if !interaction["prompt"].is_string() {
                return Err(McpError::InvalidResponse);
            }
            if !interaction["placeholder"].is_null() {
                if !interaction["placeholder"].is_string() {
                    return Err(McpError::InvalidResponse);
                }
                result.content["placeholder"] = interaction["placeholder"].clone();
            }
        }
        "select" => populate_selection(result, interaction)?,
        _ => unreachable!(),
    }
    result.invocation_id = Some(invocation.into());
    Ok(())
}

fn populate_selection(result: &mut Review, interaction: &Value) -> Result<(), McpError> {
    let rows = interaction["options"]
        .as_array()
        .ok_or(McpError::InvalidResponse)?;
    let min = interaction["min"]
        .as_u64()
        .ok_or(McpError::InvalidResponse)?;
    let max = interaction["max"]
        .as_u64()
        .ok_or(McpError::InvalidResponse)?;
    if rows.len() > 50 {
        return Err(McpError::Limit);
    }
    if min > max || min > rows.len() as u64 {
        return Err(McpError::InvalidResponse);
    }
    let mut options = Vec::new();
    for row in rows {
        let id = reference(&row["asset_id"], "artifact")?;
        if row["state"] != "ready"
            || !matches!(row["type"].as_str(), Some("image" | "video" | "model3d"))
            || result.assets.iter().any(|asset| asset["id"] == id)
        {
            return Err(McpError::InvalidResponse);
        }
        let url = public_url(&row["url"])?;
        let thumbnail = public_url(&row["thumbnail_url"])?;
        options.push(json!({"asset_id":row["asset_id"],"type":row["type"],"state":"ready","url":url,"thumbnail_url":thumbnail}));
        // Options are already ready supplier-owned assets, not review targets needing another view call.
        result
            .assets
            .push(json!({"id":id,"kind":row["type"],"state":"ready","url":url}));
    }
    result.content["options"] = json!(options);
    result.content["min"] = json!(min);
    result.content["max"] = json!(max);
    Ok(())
}

fn public_url(value: &Value) -> Result<Option<String>, McpError> {
    if value.is_null() {
        return Ok(None);
    }
    let raw = value.as_str().ok_or(McpError::InvalidResponse)?;
    crate::http_client::public_asset_url::checked(raw)
        .map(|url| Some(url.to_string()))
        .map_err(|_| McpError::InvalidResponse)
}
