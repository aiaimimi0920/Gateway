//! Web attachment upload uses only the fixed Google upload service, without account headers.
use std::time::Duration;

use futures::StreamExt;
use rquest::{Client, Method};
use serde_json::{json, Value};

use crate::error::{classify_network_error, GatewayError};
use crate::protocol::gemini::web_reverse::{GeminiWebBootstrap, WebUpload};

pub(super) const UPLOAD_URL: &str = "https://content-push.googleapis.com/upload";

fn upload_error() -> GatewayError {
    GatewayError::server_error("Gemini Web attachment upload returned no valid file reference.")
        .with_provider(super::PROVIDER)
        .with_code("gemini_web_upload_failed")
}

pub(super) async fn upload_files(
    http: &Client,
    timeout: Duration,
    bootstrap: &GeminiWebBootstrap,
    files: Vec<WebUpload>,
    upload_url: &str,
) -> Result<Vec<Value>, GatewayError> {
    if files.is_empty() {
        return Ok(Vec::new());
    }
    let push_id = bootstrap
        .push_id
        .as_deref()
        .filter(|s| !s.is_empty() && s.len() <= 4096 && s.bytes().all(|c| c.is_ascii_graphic()))
        .ok_or_else(|| {
            GatewayError::bad_request("Gemini Web upload requires the current app's Push-ID.")
                .with_code("gemini_web_upload_bootstrap_missing")
        })?;
    let mut references = Vec::with_capacity(files.len());
    // Sequential ownership bounds the extra multipart allocation to one admitted file.
    let deadline = tokio::time::Instant::now() + timeout.max(Duration::from_secs(120));
    for file in files {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            return Err(upload_error());
        }
        let boundary = format!("gateway-{}", uuid::Uuid::new_v4());
        if file
            .bytes
            .windows(boundary.len())
            .any(|w| w == boundary.as_bytes())
        {
            return Err(upload_error());
        }
        let mut body = format!("--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{}\"\r\nContent-Type: {}\r\n\r\n", file.file_name, file.mime_type).into_bytes();
        body.extend_from_slice(&file.bytes);
        body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
        // Do not copy Cookie, Authorization, model headers or caller-supplied headers
        // across origins. The upload contract is Push-ID plus bard-storage tenant.
        let response = http
            .request(Method::POST, upload_url)
            .header("origin", "https://gemini.google.com")
            .header("referer", "https://gemini.google.com/")
            .header(
                "user-agent",
                crate::protocol::gemini::web_reverse::GEMINI_WEB_DEFAULT_USER_AGENT,
            )
            .header("x-tenant-id", "bard-storage")
            .header("push-id", push_id)
            .header(
                "content-type",
                format!("multipart/form-data; boundary={boundary}"),
            )
            .redirect(rquest::redirect::Policy::none())
            .timeout(remaining)
            .body(body)
            .send()
            .await
            .map_err(|e| classify_network_error(&e, Some(super::PROVIDER)))?;
        if !response.status().is_success() {
            return Err(upload_error());
        }
        let mut stream = response.bytes_stream();
        let mut bytes = Vec::new();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| classify_network_error(&e, Some(super::PROVIDER)))?;
            if bytes.len().saturating_add(chunk.len()) > 4096 {
                return Err(upload_error());
            }
            bytes.extend_from_slice(&chunk);
        }
        let reference = std::str::from_utf8(&bytes)
            .map_err(|_| upload_error())?
            .trim();
        if !reference.starts_with("/contrib_service/")
            || reference.contains("..")
            || !reference
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'/' | b'_' | b'-' | b'.'))
        {
            return Err(upload_error());
        }
        references.push(json!([[reference], file.file_name]));
    }
    Ok(references)
}
