//! Single-attempt, fixed-origin Bearer HTTP with cumulative decoded metadata bounds.
use crate::{
    error::{ErrorKind, GatewayError},
    protocol::luma_agents::{self as contract, failure, input, BASE},
    routing::candidate::ProviderAccountPayload,
};
use futures::StreamExt;
use rquest::header::{HeaderMap, HeaderValue};
use serde_json::Value;
use std::time::Duration;

pub(super) struct Material {
    headers: HeaderMap,
}
impl Material {
    pub(super) fn headers(&self) -> HeaderMap {
        self.headers.clone()
    }
    pub fn new(payload: &ProviderAccountPayload) -> Result<Self, GatewayError> {
        contract::validate(payload, payload.canonical_adapter())?;
        if payload.adapter != contract::ADAPTER
            || payload.extra_body.as_ref().is_some_and(|v| !v.is_empty())
            || !contract::valid_key(&payload.api_key)
        {
            return Err(input(
                "luma_agents_material",
                "Luma Agents 需要显式 API Key；不接受 Web 会话或任意运行配置。",
            ));
        }
        let auth = HeaderValue::from_str(&format!("Bearer {}", payload.api_key)).map_err(|_| {
            input(
                "luma_agents_material",
                "Luma Agents API Key 不是有效的 header 材料。",
            )
        })?;
        // Product-owned headers only. Caller/provider default cookies and auth never enter this map.
        let mut headers = HeaderMap::new();
        headers.insert("authorization", auth);
        headers.insert("accept", HeaderValue::from_static("application/json"));
        Ok(Self { headers })
    }
}

pub(super) async fn json(
    client: &rquest::Client,
    material: &Material,
    id: Option<&str>,
    body: Option<&Value>,
    remaining: Duration,
    total_bytes: &mut usize,
) -> Result<Value, GatewayError> {
    let url = match id {
        Some(id) => format!("{BASE}/generations/{id}"),
        None => format!("{BASE}/generations"),
    };
    #[cfg(test)]
    let url = super::test_support::target(&url);
    let request = if let Some(body) = body {
        client.post(&url).json(body)
    } else {
        client.get(&url)
    };
    let response = request
        .default_headers(false)
        .headers(material.headers.clone())
        .redirect(rquest::redirect::Policy::none())
        .timeout(remaining)
        .send()
        .await
        .map_err(network)?;
    let expected = if body.is_some() { 201 } else { 200 };
    admit_status(response.status().as_u16(), expected)?;
    let mime = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .split(';')
        .next()
        .unwrap_or("")
        .trim();
    if !mime.eq_ignore_ascii_case("application/json") {
        return Err(failure(
            ErrorKind::ServerError,
            502,
            "luma_agents_content_type",
            "Luma Agents 未返回 JSON；正文已省略。",
        ));
    }
    let bytes = read(response, 64 * 1024).await?;
    *total_bytes = total_bytes
        .checked_add(bytes.len())
        .filter(|n| *n <= 4 * 1024 * 1024)
        .ok_or_else(body_limit)?;
    serde_json::from_slice(&bytes).map_err(|_| {
        failure(
            ErrorKind::ServerError,
            502,
            "luma_agents_json",
            "Luma Agents 返回无效 JSON；正文已省略。",
        )
    })
}

pub(super) async fn read(
    response: rquest::Response,
    limit: usize,
) -> Result<Vec<u8>, GatewayError> {
    if response.content_length().is_some_and(|n| n > limit as u64) {
        return Err(body_limit());
    }
    let mut stream = response.bytes_stream();
    let mut bytes = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(network)?;
        let next = bytes
            .len()
            .checked_add(chunk.len())
            .filter(|n| *n <= limit)
            .ok_or_else(body_limit)?;
        bytes
            .try_reserve(next - bytes.len())
            .map_err(|_| body_limit())?;
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}
pub(super) fn admit_status(status: u16, expected: u16) -> Result<(), GatewayError> {
    if status == expected {
        return Ok(());
    }
    let kind = match status {
        401 | 403 => ErrorKind::Authentication,
        429 => ErrorKind::RateLimit,
        400 | 422 => ErrorKind::BadRequest,
        _ => ErrorKind::ServerError,
    };
    Err(failure(
        kind,
        public_failure_status(status),
        "luma_agents_http",
        "Luma Agents HTTP 请求失败；不重提生成，上游正文已省略。",
    ))
}
pub(super) fn public_failure_status(status: u16) -> u16 {
    if (400..=599).contains(&status) {
        status
    } else {
        502
    }
}
pub(super) fn network(error: rquest::Error) -> GatewayError {
    if error.is_timeout() {
        timeout_error()
    } else {
        failure(
            ErrorKind::Network,
            502,
            "luma_agents_network",
            "Luma Agents 网络失败；不重提生成，连接详情已省略。",
        )
    }
}
pub(super) fn timeout_error() -> GatewayError {
    failure(
        ErrorKind::Timeout,
        504,
        "luma_agents_timeout",
        "Luma Agents 总时限已用尽；远端任务可能仍在运行，不自动重提。",
    )
}
pub(super) fn body_limit() -> GatewayError {
    failure(
        ErrorKind::ServerError,
        502,
        "luma_agents_body_limit",
        "Luma Agents 响应超过本地累计字节预算。",
    )
}
