//! The async capability returns a task ID, never authorization to submit a second generation.
use super::{failure, transport};
use crate::{error::GatewayError, routing::candidate::ProviderAccountPayload};
use rquest::header::HeaderMap;
use serde_json::Value;

pub(super) async fn finish(
    client: &rquest::Client,
    payload: &ProviderAccountPayload,
    headers: &HeaderMap,
    body: Value,
    deadline: tokio::time::Instant,
    total: &mut usize,
) -> Result<Value, GatewayError> {
    let Some(status) = body.get("status") else {
        return Ok(body);
    };
    if status.as_str() != Some("pending") {
        return Err(invalid());
    }
    let id = body
        .get("jobId")
        .and_then(Value::as_str)
        .filter(|id| {
            !id.is_empty()
                && id.len() <= 128
                && id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
        })
        .ok_or_else(invalid)?;
    let path = format!("/api/jobs/{id}");
    loop {
        let body =
            transport::send(client, payload, headers, &path, None, deadline, total, true).await?;
        if body.get("jobId").is_some_and(|v| v.as_str() != Some(id)) {
            return Err(invalid());
        }
        match body.get("status").and_then(Value::as_str) {
            Some("completed") => {
                return body
                    .get("data")
                    .filter(|v| v.is_object() || v.is_array())
                    .cloned()
                    .ok_or_else(invalid)
            }
            Some("error") => {
                return Err(failure(
                    "chataibot_job_failed",
                    "ChatAIBot 图片任务失败；上游消息和材料已省略。",
                ))
            }
            Some("pending") => tokio::time::sleep(std::time::Duration::from_secs(2)).await,
            _ => return Err(invalid()),
        }
    }
}
fn invalid() -> GatewayError {
    failure(
        "chataibot_job_invalid",
        "ChatAIBot 图片任务标识或状态无效；不猜测成功、不自动重提。",
    )
}
