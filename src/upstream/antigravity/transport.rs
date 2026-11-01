//! Bounded native HTTP; fixed Google hosts, no redirects, no automatic generation replay.
use super::{failure, fingerprint, native};
use crate::{error::GatewayError, protocol::antigravity::BASE_URL};
use futures::StreamExt;
use serde_json::Value;
use std::time::Duration;

pub(super) fn admit_base(base: &str) -> Result<&str, GatewayError> {
    let base = base.trim_end_matches('/');
    if matches!(
        base,
        BASE_URL | "https://cloudcode-pa.googleapis.com/v1internal"
    ) {
        Ok(base)
    } else {
        Err(failure(
            "antigravity_origin_invalid",
            "Antigravity only accepts official Cloud Code HTTPS endpoints",
        ))
    }
}

pub(crate) async fn read(
    response: rquest::Response,
    limit: usize,
) -> Result<Vec<u8>, GatewayError> {
    let status = response.status();
    if !status.is_success() {
        return Err(failure(
            if matches!(status.as_u16(), 401 | 403) {
                "antigravity_authorization_required"
            } else {
                "antigravity_upstream_rejected"
            },
            &format!(
                "Antigravity HTTP {}; private upstream body omitted",
                status.as_u16()
            ),
        ));
    }
    if response.content_length().is_some_and(|n| n > limit as u64) {
        return Err(failure(
            "antigravity_response_limit",
            "Antigravity response exceeds size limit",
        ));
    }
    let mut stream = response.bytes_stream();
    let mut bytes = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| {
            failure(
                "antigravity_body_failed",
                "Antigravity response read failed; not replayed",
            )
        })?;
        if bytes.len().saturating_add(chunk.len()) > limit {
            return Err(failure(
                "antigravity_response_limit",
                "Antigravity response exceeds size limit",
            ));
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

pub(super) async fn post(
    client: &rquest::Client,
    base: &str,
    token: &str,
    method: &str,
    body: &Value,
    limit: usize,
) -> Result<Vec<u8>, GatewayError> {
    let base = admit_base(base)?;
    let request = fingerprint::apply(client.post(format!("{base}:{method}")), token)
        .timeout(Duration::from_secs(180))
        .json(body);
    let response = native::send(request).await?;
    read(response, limit).await
}

pub(super) async fn json(
    client: &rquest::Client,
    base: &str,
    token: &str,
    method: &str,
    body: &Value,
) -> Result<Value, GatewayError> {
    let bytes = post(client, base, token, method, body, 2 * 1024 * 1024).await?;
    serde_json::from_slice(&bytes).map_err(|_| {
        failure(
            "antigravity_invalid_json",
            "Antigravity returned invalid JSON",
        )
    })
}
