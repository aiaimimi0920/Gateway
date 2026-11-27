//! Endpoint-specific admission. Projection is private to this adapter: the pipeline
//! retains the original endpoint, scope, audit identity and response format.
use super::{chat_request, incremental, inline_images, request, runtime, tool_request};
use crate::{
    error::GatewayError,
    protocol::canonical::{CanonicalRelayRequest, EndpointKind, ProtocolFamily},
};
use std::borrow::Cow;

pub(crate) enum Execution {
    Text(TextRequest),
    Tools(tool_request::ToolRequest),
    Incremental(incremental::IncrementalRequest),
}
pub(crate) struct TextRequest {
    pub prompt: String,
    pub images: inline_images::Images,
}
impl Execution {
    pub(crate) fn prompt(&self) -> &str {
        match self {
            Self::Text(request) => &request.prompt,
            Self::Tools(request) => &request.prompt,
            Self::Incremental(_) => "",
        }
    }
    pub(crate) async fn prepare_input(
        &self,
        input: &mut serde_json::Value,
    ) -> Result<(), GatewayError> {
        let images = match self {
            Self::Text(request) => &request.images,
            Self::Tools(request) if request.output.is_none() => &request.images,
            Self::Tools(_) | Self::Incremental(_) => return Ok(()),
        };
        if !images.is_empty() {
            input["images"] = images.worker_input().await?;
        }
        Ok(())
    }
    pub(crate) fn into_tools(self) -> Option<runtime::ToolTurn> {
        match self {
            Self::Text(_) => None,
            Self::Tools(request) => Some(request.into()),
            Self::Incremental(request) => Some(request.into()),
        }
    }
}

pub(crate) fn scope(req: &CanonicalRelayRequest) -> Result<&'static str, GatewayError> {
    match (req.protocol_family, req.endpoint_kind) {
        (ProtocolFamily::OpenAi, EndpointKind::Responses) => Ok("responses"),
        (ProtocolFamily::OpenAi, EndpointKind::ChatCompletions) => Ok("chat.completions"),
        (ProtocolFamily::GeminiGenerateContent, EndpointKind::ChatCompletions) => {
            Ok("gemini.generateContent")
        }
        (ProtocolFamily::CohereChat, EndpointKind::ChatCompletions) => Ok("cohere.chat"),
        (ProtocolFamily::BedrockConverse, EndpointKind::ChatCompletions) => Ok("bedrock.converse"),
        (ProtocolFamily::DashScope, EndpointKind::ChatCompletions) => Ok("dashscope.text"),
        (ProtocolFamily::DashScopeMultimodal, EndpointKind::ChatCompletions) => {
            Ok("dashscope.multimodal")
        }
        _ => Err(crate::routing::config::cursor::stop(
            GatewayError::bad_request(
                "Cursor remote has no verified generation token limit for this endpoint",
            )
            .with_code("cursor_remote_unsupported_endpoint"),
        )),
    }
}

/// Scope paths are private to Cursor admission; other providers keep their
/// existing authorization contract. The live binding retains the original family.
pub(crate) fn auth_path(req: &CanonicalRelayRequest) -> Option<&'static str> {
    if !crate::routing::config::cursor::private_request(req.requested_model.as_deref())
        || req.endpoint_kind != EndpointKind::ChatCompletions
    {
        return None;
    }
    match req.protocol_family {
        ProtocolFamily::GeminiGenerateContent => Some("/v1/gemini/generateContent"),
        ProtocolFamily::CohereChat => Some("/v1/cohere/chat"),
        ProtocolFamily::BedrockConverse => Some("/v1/bedrock/converse"),
        ProtocolFamily::DashScope => Some("/v1/dashscope/text"),
        ProtocolFamily::DashScopeMultimodal => Some("/v1/dashscope/multimodal"),
        _ => None,
    }
}

pub(crate) fn validate_query(
    req: &CanonicalRelayRequest,
    query: &std::collections::HashMap<String, String>,
) -> Result<(), GatewayError> {
    if auth_path(req).is_some()
        && query.iter().any(|(key, value)| {
            !(req.protocol_family == ProtocolFamily::GeminiGenerateContent
                && req.stream
                && key == "alt"
                && value == "sse")
        })
    {
        return Err(request::invalid());
    }
    Ok(())
}

pub(crate) fn parse(req: &CanonicalRelayRequest) -> Result<Execution, GatewayError> {
    parse_with_continuation(req, None)
}

pub(crate) fn parse_with_continuation(
    req: &CanonicalRelayRequest,
    continuation: Option<&runtime::Continuation>,
) -> Result<Execution, GatewayError> {
    scope(req)?;
    if req.raw_body.to_string().len() > inline_images::MAX_REQUEST {
        return Err(super::tool_schema::invalid());
    }
    if req.raw_body.get("previous_response_id").is_some() {
        if req.protocol_family != ProtocolFamily::OpenAi
            || req.endpoint_kind != EndpointKind::Responses
        {
            return Err(super::tool_schema::invalid());
        }
        return incremental::parse(req, continuation.ok_or_else(incremental::unavailable)?)
            .map(Execution::Incremental);
    }
    let chat = req.endpoint_kind == EndpointKind::ChatCompletions;
    let projected = match req.protocol_family {
        ProtocolFamily::GeminiGenerateContent => Cow::Owned(super::gemini_request::project(req)?),
        ProtocolFamily::CohereChat => Cow::Owned(super::cohere_request::project(req)?),
        family if super::metered_output::supports(family) => {
            Cow::Owned(super::metered_request::project(req)?)
        }
        _ if chat => Cow::Owned(chat_request::project(req)?),
        _ => Cow::Borrowed(req),
    };
    // Inspect raw declarations, not the permissive generic normalizer's output.
    let has_tools = match projected.raw_body.get("tools") {
        None => false,
        Some(value) => !value
            .as_array()
            .ok_or_else(super::tool_schema::invalid)?
            .is_empty(),
    };
    if !has_tools {
        let (projected, images) = inline_images::extract(&projected, chat)?;
        let prompt = request::prompt_with_images(&projected, !images.is_empty())?;
        return Ok(Execution::Text(TextRequest { prompt, images }));
    }
    let mut parsed = tool_request::parse_projected(&projected, chat)?;
    parsed.family = req.protocol_family;
    if projected
        .raw_body
        .get("tool_choice")
        .is_some_and(|v| v == "none")
    {
        // parse has validated all declarations and rejected tool history.
        // No MCP definitions or tool bridge may reach the no-tools worker.
        Ok(Execution::Text(TextRequest {
            prompt: parsed.prompt,
            images: parsed.images,
        }))
    } else {
        Ok(Execution::Tools(parsed))
    }
}
