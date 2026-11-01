//! One native HTTP project/generation per call. Ambiguous failures cannot be replayed.
use super::{
    failure, material, result, session::StitchCookies, transport::StitchHttp, wire, MODEL,
};
use crate::{
    error::GatewayError,
    protocol::canonical::{
        CanonicalRelayRequest, CanonicalRelayResponse, EndpointKind, MessageRole,
    },
    routing::candidate::ProviderAccountPayload,
};
use serde_json::json;
use std::{sync::LazyLock, time::Duration};
use tokio::sync::Semaphore;
static SLOTS: LazyLock<Semaphore> = LazyLock::new(|| Semaphore::new(2));

fn prompt(req: &CanonicalRelayRequest, model: &str) -> Result<String, GatewayError> {
    if model != MODEL
        || !matches!(
            req.endpoint_kind,
            EndpointKind::ChatCompletions
                | EndpointKind::Responses
                | EndpointKind::Messages
                | EndpointKind::Completions
        )
        || !req.tools.is_empty()
        || req.previous_response_id.is_some()
        || req.messages.len() > 32
        || req.messages.iter().any(|m| {
            m.role == MessageRole::Tool
                || !m.tool_calls.is_empty()
                || m.content.iter().any(|p| p.as_text().is_none())
        })
    {
        return Err(failure(
            "stitch_web_input_unsupported",
            "Stitch 当前仅接收界面设计文本；不支持工具、附件或续接 Responses ID。",
        ));
    }
    let mut text = String::new();
    for message in &req.messages {
        let part = message.text_content();
        if text
            .len()
            .saturating_add(part.len())
            .saturating_add(usize::from(!text.is_empty()))
            > 32768
        {
            return Err(failure(
                "stitch_web_prompt_limit",
                "Stitch 设计提示超过 32 KiB。",
            ));
        }
        if !text.is_empty() {
            text.push('\n');
        }
        text.push_str(&part);
    }
    if text.trim().is_empty() {
        return Err(failure(
            "stitch_web_prompt_missing",
            "Stitch 需要非空的界面设计提示。",
        ));
    }
    Ok(text)
}

pub async fn execute(
    client: &rquest::Client,
    timeout: Duration,
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
) -> Result<CanonicalRelayResponse, GatewayError> {
    let prompt = prompt(req, model)?;
    super::admit_origin(&payload.base_url)?;
    tokio::time::timeout(timeout.min(Duration::from_secs(180)), async {
        let _permit = SLOTS
            .acquire()
            .await
            .map_err(|_| failure("stitch_web_capacity_unavailable", "Stitch 请求容量不可用。"))?;
        let bytes = material(payload).await?;
        let mut http = StitchHttp::open(client, StitchCookies::from_bundle(&bytes)?).await?;
        let created = http
            .rpc(
                wire::CREATE,
                &json!([[null, null, null, null, null, 4]]),
                "/",
            )
            .await?;
        let project = wire::project(&created)?;
        let source = format!("/{project}");
        let mut state = http
            .rpc(
                wire::GENERATE,
                &wire::generate_params(&project, &prompt),
                &source,
            )
            .await?;
        let session = wire::session(&state)?;
        if !session.starts_with(&format!("{project}/sessions/")) {
            return Err(failure(
                "stitch_web_session_mismatch",
                "Stitch 返回了不匹配的生成会话。",
            ));
        }
        for attempt in 0..45 {
            if wire::session(&state)? != session {
                return Err(failure(
                    "stitch_web_session_mismatch",
                    "Stitch 轮询结果与请求会话不匹配。",
                ));
            }
            if state.get(2).and_then(serde_json::Value::as_u64) == Some(3) {
                let screens = http.rpc(wire::SCREENS, &json!([project]), &source).await?;
                return Ok(CanonicalRelayResponse {
                    model: model.into(),
                    model_declaration: None,
                    text: result::manifest(&project, &screens)?,
                    usage: None,
                    tool_calls: vec![],
                    upstream_status: Some(200),
                    finish_reason: Some("stop".into()),
                });
            }
            tokio::time::sleep(Duration::from_secs(if attempt < 3 { 2 } else { 4 })).await;
            state = http.rpc(wire::POLL, &json!([session]), &source).await?;
        }
        Err(failure(
            "stitch_web_generation_pending",
            "Stitch 生成尚未完成；未将待处理任务记为成功，也不会重新提交。",
        ))
    })
    .await
    .map_err(|_| {
        failure(
            "stitch_web_deadline_exceeded",
            "Stitch 请求超时；可能已创建项目或提交生成，不会自动重放。",
        )
    })?
}
