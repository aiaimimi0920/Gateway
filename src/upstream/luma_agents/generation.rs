//! One generation ID, bounded polling, then verified asset reads; no automatic POST replay.
use super::{
    assets,
    transport::{self, Material},
};
use crate::{
    error::{ErrorKind, GatewayError},
    protocol::luma_agents::{failure, generation_audit, GenerationRequest, Media},
};
use base64::Engine;
use serde_json::{json, Value};
use std::time::Duration;

pub(super) async fn run(
    client: &rquest::Client,
    material: &Material,
    request: GenerationRequest,
    duration: Duration,
) -> Result<Value, GatewayError> {
    let deadline = tokio::time::Instant::now() + duration;
    let mut total = 0;
    let mut job = transport::json(
        client,
        material,
        None,
        Some(&request.body),
        duration,
        &mut total,
    )
    .await?;
    let id = read_id(&job)?;
    let mut polls = 0usize;
    loop {
        let state = validate(&job, &request, &id)?;
        generation_audit::observe(
            &id,
            &request.model,
            request.kind,
            state,
            job["usage"]["credits_charged"].as_number().cloned(),
            false,
        )
        .await?;
        match job["state"].as_str() {
            Some("completed") => break,
            Some("failed") => {
                // Fix the original failure classification before optional accounting reads.
                let mut error = failed(&job);
                let usage = super::settlement::collect(
                    client, material, &request, &id, &job, deadline, &mut total,
                )
                .await?;
                error.luma_generation_failure =
                    Some(Box::new(crate::error::LumaGenerationFailure::verified(
                        id,
                        request.model,
                        request.kind,
                        usage["credits_charged"].as_number().cloned(),
                    )));
                return Err(error);
            }
            Some("queued" | "processing") => (),
            _ => return Err(invalid()),
        }
        if polls >= 300 {
            return Err(transport::timeout_error());
        }
        #[cfg(not(test))]
        let interval = Duration::from_secs(if request.media == Media::Image { 2 } else { 5 });
        #[cfg(test)]
        let interval = Duration::from_millis(5);
        tokio::time::sleep(interval).await;
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            return Err(transport::timeout_error());
        }
        job = transport::json(client, material, Some(&id), None, remaining, &mut total).await?;
        polls += 1;
    }
    let outputs = job
        .get("output")
        .and_then(Value::as_array)
        .filter(|v| !v.is_empty() && v.len() <= if request.kind == "layering" { 10 } else { 9 })
        .ok_or_else(invalid)?;
    let layers = if request.kind == "layering" {
        Some(super::layering_output::stack(outputs)?)
    } else {
        None
    };
    let mut data = Vec::with_capacity(outputs.len());
    let mut asset_bytes = 0usize;
    for (index, output) in outputs.iter().enumerate() {
        let media = if request.media == Media::Image {
            "image"
        } else {
            "video"
        };
        if output.get("type").and_then(Value::as_str) != Some(media)
            || layers.is_none() && output.get("layer").is_some_and(|v| !v.is_null())
        {
            return Err(invalid());
        }
        let url = output
            .get("url")
            .and_then(Value::as_str)
            .ok_or_else(invalid)?;
        let limit = 64 * 1024 * 1024 - asset_bytes;
        let (bytes, mime) = if let Some(layers) = &layers {
            super::layering_output::download(url, &layers[index], deadline, limit).await?
        } else {
            assets::download(url, request.media, deadline, limit).await?
        };
        asset_bytes += bytes.len();
        let mut value = if request.media == Media::Image {
            if request.base64 {
                json!({"b64_json":base64::engine::general_purpose::STANDARD.encode(bytes),"mime_type":mime})
            } else {
                json!({"url":url,"mime_type":mime})
            }
        } else {
            json!({"url":if request.base64 {Value::Null}else{json!(url)},"mime_type":mime,
                "body_base64":if request.base64 {json!(base64::engine::general_purpose::STANDARD.encode(bytes))}else{Value::Null}})
        };
        if let Some(layers) = &layers {
            value["layer"] =
                serde_json::to_value(&layers[index]).expect("validated layer metadata serializes");
        }
        data.push(value);
    }
    let created = job
        .get("created_at")
        .and_then(Value::as_str)
        .and_then(|s| {
            time::OffsetDateTime::parse(s, &time::format_description::well_known::Rfc3339).ok()
        })
        .ok_or_else(invalid)?
        .unix_timestamp();
    // Optional accounting must not consume the time needed to verify the media itself.
    generation_audit::observe(
        &id,
        &request.model,
        request.kind,
        "completed",
        job["usage"]["credits_charged"].as_number().cloned(),
        true,
    )
    .await?;
    let usage =
        super::settlement::collect(client, material, &request, &id, &job, deadline, &mut total)
            .await?;
    let mut result = json!({"created":created,"data":data,"provider":"lumalabs_agents_api",
        "gateway_luma_generation":{"id":id,"model":request.model,"type":request.kind,"state":"completed",
            "usage":usage}});
    if request.media == Media::Video {
        result["object"] = json!("video.generation");
        result["completed"] = json!(true);
        result["model"] = json!(request.model);
    }
    Ok(result)
}
fn read_id(job: &Value) -> Result<String, GatewayError> {
    let id = job
        .get("id")
        .and_then(Value::as_str)
        .filter(|s| s.len() == 36)
        .ok_or_else(invalid)?;
    uuid::Uuid::parse_str(id)
        .map(|id| id.to_string())
        .map_err(|_| invalid())
}
pub(super) fn validate(
    job: &Value,
    request: &GenerationRequest,
    id: &str,
) -> Result<&'static str, GatewayError> {
    validate_identity(job, &request.model, request.kind, id)
}
pub(super) fn validate_identity(
    job: &Value,
    model: &str,
    kind: &str,
    id: &str,
) -> Result<&'static str, GatewayError> {
    if !job.is_object()
        || read_id(job)? != id
        || job.get("model").and_then(Value::as_str) != Some(model)
        || job.get("type").and_then(Value::as_str) != Some(kind)
    {
        return Err(invalid());
    }
    usage(job)?;
    if job.get("state").and_then(Value::as_str) != Some("failed")
        && ["failure_code", "failure_reason"]
            .iter()
            .any(|key| job.get(*key).is_some_and(|v| !v.is_null()))
    {
        return Err(invalid());
    }
    if job.get("state").and_then(Value::as_str) != Some("completed")
        && job
            .get("output")
            .is_some_and(|v| !v.is_null() && !v.as_array().is_some_and(Vec::is_empty))
    {
        return Err(invalid());
    }
    match job["state"].as_str() {
        Some("queued") => Ok("queued"),
        Some("processing") => Ok("processing"),
        Some("completed") => Ok("completed"),
        Some("failed") => Ok("failed"),
        _ => Err(invalid()),
    }
}
pub(super) fn usage(job: &Value) -> Result<Value, GatewayError> {
    let Some(usage) = job.get("usage").filter(|v| !v.is_null()) else {
        return Ok(json!({"credits_charged":null}));
    };
    let usage = usage.as_object().ok_or_else(invalid)?;
    let amount = usage.get("credits_charged").unwrap_or(&Value::Null);
    if !amount.is_null() && !amount.as_f64().is_some_and(|n| n.is_finite() && n >= 0.0) {
        return Err(invalid());
    }
    Ok(json!({"credits_charged":amount}))
}
fn invalid() -> GatewayError {
    failure(
        ErrorKind::ServerError,
        502,
        "luma_agents_generation",
        "Luma Agents 任务身份、状态或输出不完整；没有按成功处理。",
    )
}
fn failed(job: &Value) -> GatewayError {
    let (kind, code) = match job.get("failure_code").and_then(Value::as_str) {
        Some("content_moderated") => (ErrorKind::ContentFilter, "luma_agents_content_moderated"),
        Some("budget_exhausted") => (ErrorKind::InsufficientQuota, "luma_agents_budget_exhausted"),
        Some("rate_limited") => (ErrorKind::RateLimit, "luma_agents_rate_limited"),
        _ => (ErrorKind::ServerError, "luma_agents_generation_failed"),
    };
    failure(
        kind,
        502,
        code,
        "Luma Agents 生成失败；私人原因已省略，不自动重提。",
    )
}
