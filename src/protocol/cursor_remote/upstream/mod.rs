//! Pure-Rust Cursor remote qualification, billing, and Connect+Protobuf execution core.
//! Zero external processes, Node.js, Cursor CLI, or third-party SDK dependencies.
pub(crate) mod connect_stream;
pub(crate) mod json_wire;
pub(crate) mod protobuf;
pub(crate) mod qualification;
pub(crate) mod selection;
pub(crate) mod text_session;
pub(crate) mod tool_stream;
pub(crate) mod transport;
pub(crate) mod transport_live;
mod usage;

use crate::{
    error::GatewayError,
    protocol::cursor_remote::runtime::worker::{WorkerHooks, WorkerOutput},
};
use base64::{engine::general_purpose::STANDARD, Engine};
use bytes::Bytes;
use connect_stream::TextDecoderState;
use serde_json::Value;
use std::time::Duration;
use text_session::text_request;
use transport::{CheckedClient, DuplexRequest, Transport, ENDPOINTS};

const MAX_INLINE_IMAGE_BASE64: usize = 1_398_108;
const MAX_INLINE_IMAGE_BYTES: usize = 1024 * 1024;
const MAX_TOTAL_IMAGE_BYTES: usize = 2 * 1024 * 1024;

#[derive(Debug, Clone)]
pub(crate) struct ProbeError {
    pub(crate) code: &'static str,
    pub(crate) status: Option<u16>,
}

impl ProbeError {
    pub(crate) const fn new(code: &'static str) -> Self {
        Self { code, status: None }
    }

    pub(crate) const fn with_status(code: &'static str, status: u16) -> Self {
        Self {
            code,
            status: Some(status),
        }
    }
}

pub(crate) enum RunError {
    Probe(ProbeError),
    Gateway(GatewayError),
}

impl From<ProbeError> for RunError {
    fn from(err: ProbeError) -> Self {
        Self::Probe(err)
    }
}

impl RunError {
    pub(crate) fn into_gateway(self) -> GatewayError {
        match self {
            Self::Probe(err) => super::error(err.code),
            Self::Gateway(err) => err,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ValidatedImage {
    pub(crate) mime: String,
    pub(crate) bytes: Vec<u8>,
}

#[derive(Debug, Clone)]
pub(crate) struct ToolSpec {
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) parameters: Value,
}

#[derive(Debug, Clone)]
pub(crate) struct Input {
    pub(crate) token: String,
    pub(crate) email_hash: String,
    pub(crate) prompt: String,
    pub(crate) proxy_port: u16,
    pub(crate) images: Vec<ValidatedImage>,
    pub(crate) tools: Vec<ToolSpec>,
}

pub(crate) fn validate_images(raw: Option<&Value>) -> Result<Vec<ValidatedImage>, ProbeError> {
    let Some(raw) = raw else {
        return Ok(Vec::new());
    };
    let invalid = || ProbeError::new("invalid_inline_image");
    let items = raw
        .as_array()
        .filter(|a| a.len() <= 4)
        .ok_or_else(invalid)?;
    let mut out = Vec::with_capacity(items.len());
    let mut total = 0usize;
    for item in items {
        let obj = item.as_object().ok_or_else(invalid)?;
        if obj.keys().any(|k| !matches!(k.as_str(), "mime" | "data")) {
            return Err(invalid());
        }
        let mime = obj
            .get("mime")
            .and_then(Value::as_str)
            .filter(|&m| matches!(m, "image/png" | "image/jpeg"))
            .ok_or_else(invalid)?;
        let data = obj
            .get("data")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty() && s.len() <= MAX_INLINE_IMAGE_BASE64)
            .ok_or_else(invalid)?;
        let decoded = STANDARD.decode(data).map_err(|_| invalid())?;
        if STANDARD.encode(&decoded) != data
            || decoded.is_empty()
            || decoded.len() > MAX_INLINE_IMAGE_BYTES
        {
            return Err(invalid());
        }
        total += decoded.len();
        if total > MAX_TOTAL_IMAGE_BYTES {
            return Err(invalid());
        }
        let magic_ok = if mime == "image/png" {
            decoded.starts_with(&[137, 80, 78, 71, 13, 10, 26, 10])
        } else {
            decoded.starts_with(&[255, 216, 255])
        };
        if !magic_ok {
            return Err(invalid());
        }
        out.push(ValidatedImage {
            mime: mime.to_owned(),
            bytes: decoded,
        });
    }
    Ok(out)
}

impl Input {
    pub(crate) fn parse(raw: &Value, proxy_port: u16) -> Result<Self, ProbeError> {
        let images = validate_images(raw.get("images"))?;
        let invalid = || ProbeError::new("invalid_runtime_input");
        let prompt = raw
            .get("prompt")
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty() && s.len() <= 65_536)
            .ok_or_else(invalid)?;
        let email_hash = raw
            .get("emailHash")
            .and_then(Value::as_str)
            .filter(|s| s.len() == 64 && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')))
            .ok_or_else(invalid)?;
        let token = raw
            .get("token")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| ProbeError::new("invalid_login_token"))?;

        let mut tools = Vec::new();
        if let Some(raw_tools) = raw.get("tools") {
            let arr = raw_tools
                .as_array()
                .ok_or_else(|| ProbeError::new("invalid_mcp_message"))?;
            for entry in arr {
                let obj = entry
                    .as_object()
                    .ok_or_else(|| ProbeError::new("invalid_mcp_message"))?;
                let name = obj
                    .get("name")
                    .and_then(Value::as_str)
                    .ok_or_else(|| ProbeError::new("invalid_mcp_message"))?;
                let description = match obj.get("description") {
                    None => String::new(),
                    Some(v) => v
                        .as_str()
                        .ok_or_else(|| ProbeError::new("invalid_mcp_message"))?
                        .to_owned(),
                };
                let parameters = obj
                    .get("parameters")
                    .cloned()
                    .ok_or_else(|| ProbeError::new("invalid_mcp_message"))?;
                tools.push(ToolSpec {
                    name: name.to_owned(),
                    description,
                    parameters,
                });
            }
        }
        Ok(Self {
            token: token.to_owned(),
            email_hash: email_hash.to_owned(),
            prompt: prompt.to_owned(),
            proxy_port,
            images,
            tools,
        })
    }
}

pub(crate) async fn prepare<'a>(
    input: &Input,
    client: &CheckedClient<'a>,
) -> Result<String, ProbeError> {
    qualification::verify_identity(&input.token, &input.email_hash, client).await?;
    let (catalog_res, config_res) = tokio::join!(
        selection::read_agent_catalog(&input.token, client),
        selection::verify_server_route(&input.token, client),
    );
    let catalog = catalog_res?;
    config_res?;
    selection::verify_model_selection(&input.token, &catalog, client).await?;
    let model = selection::require_account_model(&catalog)?;
    qualification::verify_free_account(&input.token, &input.email_hash, client).await?;
    Ok(model)
}

pub(crate) async fn execute(
    input: Input,
    hooks: &mut WorkerHooks,
    transport: &dyn Transport,
) -> Result<WorkerOutput, RunError> {
    let client = CheckedClient::new(input.proxy_port, transport);
    let model = prepare(&input, &client).await?;
    if !input.tools.is_empty() {
        return tool_stream::execute_tools(
            &input.token,
            model,
            &input.prompt,
            &input.images,
            &input.tools,
            &client,
            hooks,
        )
        .await;
    }
    let mut decoder = TextDecoderState::new_text()?;
    let body = text_request(&model, &input.prompt, 64 * 1024, &input.images)?;
    let auth_header = format!("Bearer {}", input.token);
    let headers = [
        ("authorization", auth_header.as_str()),
        ("content-type", "application/connect+proto"),
        ("accept", "application/connect+proto"),
        ("connect-protocol-version", "1"),
        ("connect-accept-encoding", "identity"),
    ];
    let mut stream = client
        .open_duplex(
            DuplexRequest {
                url: ENDPOINTS.run,
                headers: &headers,
                body: Bytes::from(body),
                timeout: Duration::from_secs(60),
            },
            hooks,
        )
        .await?;
    while let Some(chunk) = stream.next_chunk().await? {
        if stream.status() == 200 {
            decoder.push(&chunk)?;
            for reply in decoder.take_replies()? {
                stream.send_frame(Bytes::from(reply))?;
            }
        }
    }
    if stream.status() != 200 {
        return Err(ProbeError::with_status("inference_http_error", stream.status()).into());
    }
    decoder.finish(model).map_err(RunError::Probe)
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tool_stream_tests;
