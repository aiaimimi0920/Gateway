//! One pull-driven HTTP body owns the invocation, total deadline, terminal and cancellation.
use super::{
    events::{Action, Decoder},
    failure, material, request,
};
use crate::{
    error::{ErrorKind, GatewayError},
    protocol::{
        canonical::{CanonicalRelayRequest, CanonicalRelayResponse},
        stream_error::{ProtocolStreamError, StreamError},
    },
    routing::candidate::ProviderAccountPayload,
    upstream::response_types::UpstreamStreamingResponse,
};
use bytes::Bytes;
use futures::{Stream, StreamExt};
use serde_json::{json, Value};
use std::{pin::Pin, time::Duration};
use tokio::time::Instant;
type Body = Pin<Box<dyn Stream<Item = Result<Bytes, rquest::Error>> + Send>>;
struct Turn {
    body: Body,
    decoder: Decoder,
    deadline: Instant,
    eof: bool,
    model: String,
    id: String,
}
impl Turn {
    async fn open(
        client: &rquest::Client,
        timeout: Duration,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
    ) -> Result<Self, GatewayError> {
        let prompt = request::validate(req, model)?;
        let mut headers = material::headers(payload)?;
        if timeout.is_zero() {
            return Err(expired());
        }
        let deadline = Instant::now() + timeout.min(Duration::from_secs(120));
        let chat = tokio::time::timeout_at(
            deadline.min(Instant::now() + Duration::from_secs(15)),
            async {
                let response = client
                    .post(endpoint("/api/v2/chats/new"))
                    .headers(headers.clone())
                    .redirect(rquest::redirect::Policy::none())
                    .timeout(deadline.saturating_duration_since(Instant::now()))
                    .json(&request::create(model))
                    .send()
                    .await
                    .map_err(network)?;
                admit(response.status().as_u16())?;
                let mut body = response.bytes_stream();
                let mut bytes = Vec::new();
                while let Some(chunk) = body.next().await {
                    let chunk = chunk.map_err(network)?;
                    if chunk.len() > 64 * 1024 - bytes.len() {
                        return Err(failure(
                            "qwen_web_current_create_limit",
                            ErrorKind::ServerError,
                        ));
                    }
                    bytes.extend_from_slice(&chunk);
                }
                let value: Value =
                    serde_json::from_slice(&bytes).map_err(|_| super::events::protocol())?;
                if value["success"] == false || value.get("error").is_some_and(|v| !v.is_null()) {
                    return Err(super::events::protocol());
                }
                value["data"]["id"]
                    .as_str()
                    .filter(|id| {
                        !id.is_empty()
                            && id.len() <= 256
                            && id
                                .bytes()
                                .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
                    })
                    .map(str::to_owned)
                    .ok_or_else(super::events::protocol)
            },
        )
        .await
        .map_err(|_| expired())??;
        headers.insert(
            "referer",
            rquest::header::HeaderValue::from_str(&format!("https://chat.qwen.ai/c/{chat}"))
                .map_err(|_| super::events::protocol())?,
        );
        let response = tokio::time::timeout_at(
            deadline.min(Instant::now() + Duration::from_secs(30)),
            client
                .post(endpoint("/api/v2/chat/completions"))
                .query(&[("chat_id", &chat)])
                .headers(headers)
                .redirect(rquest::redirect::Policy::none())
                .timeout(deadline.saturating_duration_since(Instant::now()))
                .json(&request::completion(model, &chat, prompt))
                .send(),
        )
        .await
        .map_err(|_| expired())?
        .map_err(network)?;
        admit(response.status().as_u16())?;
        if !response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .is_some_and(|v| {
                v.split(';')
                    .next()
                    .is_some_and(|v| v.trim().eq_ignore_ascii_case("text/event-stream"))
            })
        {
            return Err(failure(
                "qwen_web_current_content_type_invalid",
                ErrorKind::ServerError,
            ));
        }
        Ok(Self {
            body: Box::pin(response.bytes_stream()),
            decoder: Decoder::new(chat),
            deadline,
            eof: false,
            model: model.into(),
            id: format!("chatcmpl-{}", uuid::Uuid::new_v4()),
        })
    }
    async fn next(&mut self) -> Result<Option<String>, GatewayError> {
        let deadline = self.deadline;
        tokio::time::timeout_at(deadline, async {
            loop {
                if let Some(action) = self.decoder.next()? {
                    return Ok(match action {
                        Action::Text(text) => Some(text),
                        Action::Done => None,
                    });
                }
                if self.eof {
                    return Err(failure(
                        "qwen_web_current_incomplete",
                        ErrorKind::ServerError,
                    ));
                }
                match self.body.next().await {
                    Some(Ok(bytes)) => self.decoder.feed(bytes)?,
                    Some(Err(error)) => return Err(network(error)),
                    None => {
                        self.eof = true;
                        self.decoder.end();
                    }
                }
            }
        })
        .await
        .map_err(|_| expired())?
    }
    fn chunk(&self, text: Option<String>) -> Bytes {
        let terminal = text.is_none();
        let mut chunk = json!({"id":self.id,"object":"chat.completion.chunk","model":self.model,"choices":[{"index":0,"delta":text.map(|s| json!({"content":s})).unwrap_or_else(|| json!({})),"finish_reason":if terminal { json!("stop") } else { Value::Null }}]});
        if terminal {
            if let Some(usage) = &self.decoder.usage {
                chunk["usage"] = json!({"prompt_tokens":usage.prompt_tokens,"completion_tokens":usage.completion_tokens,"total_tokens":usage.total_tokens});
            }
        }
        Bytes::from(format!(
            "data: {chunk}\n\n{}",
            if terminal { "data: [DONE]\n\n" } else { "" }
        ))
    }
}
pub(crate) async fn execute(
    client: &rquest::Client,
    timeout: Duration,
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
) -> Result<CanonicalRelayResponse, GatewayError> {
    let mut turn = Turn::open(client, timeout, payload, req, model).await?;
    let mut text = String::new();
    while let Some(delta) = turn.next().await? {
        text.push_str(&delta);
    }
    Ok(CanonicalRelayResponse {
        bedrock_turn: None,
        vertex_turn: None,
        model_declaration: None,
        model: model.into(),
        text,
        sources: Vec::new(),
        usage: turn.decoder.usage,
        tool_calls: Vec::new(),
        upstream_status: Some(200),
        finish_reason: Some("stop".into()),
    })
}
pub(crate) async fn execute_stream(
    client: &rquest::Client,
    timeout: Duration,
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
) -> Result<UpstreamStreamingResponse, GatewayError> {
    let turn = Turn::open(client, timeout, payload, req, model).await?;
    let stream = futures::stream::unfold(Some(turn), |turn| async move {
        let mut turn = turn?;
        match turn.next().await {
            Ok(Some(text)) => Some((Ok(turn.chunk(Some(text))), Some(turn))),
            Ok(None) => Some((Ok(turn.chunk(None)), None)),
            Err(error) => Some((
                Err(StreamError::<rquest::Error>::Protocol(
                    ProtocolStreamError::invalid_data(
                        error
                            .code
                            .as_deref()
                            .unwrap_or("qwen_web_current_stream_failed"),
                    ),
                )),
                None,
            )),
        }
    });
    Ok(UpstreamStreamingResponse::NativeEvents(Box::pin(stream)))
}
fn endpoint(path: &str) -> String {
    #[cfg(test)]
    {
        return super::test_support::target(path);
    }
    #[cfg(not(test))]
    {
        format!("https://chat.qwen.ai{path}")
    }
}
fn admit(status: u16) -> Result<(), GatewayError> {
    if (200..300).contains(&status) {
        return Ok(());
    }
    let (code, kind) = match status {
        401 | 403 => ("qwen_web_current_authentication", ErrorKind::Authentication),
        429 => ("qwen_web_current_rate_limit", ErrorKind::RateLimit),
        300..=399 => ("qwen_web_current_redirect", ErrorKind::ServerError),
        _ => ("qwen_web_current_http_failed", ErrorKind::ServerError),
    };
    let mut error = failure(code, kind);
    error.http_status = Some(status);
    Err(error)
}
fn expired() -> GatewayError {
    failure("qwen_web_current_timeout", ErrorKind::Timeout)
}
fn network(error: rquest::Error) -> GatewayError {
    if error.is_timeout() {
        expired()
    } else {
        failure("qwen_web_current_network", ErrorKind::Network)
    }
}
