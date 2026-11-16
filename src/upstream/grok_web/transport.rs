//! One socket/future owns the turn. Backpressure, timeout and Drop never start a replay task.
use super::{
    dial, failure,
    machine::{Action, Machine},
    request, session,
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
use futures::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::time::Duration;
use tokio::time::Instant;
use tokio_tungstenite::tungstenite::Message;

struct Turn {
    socket: dial::Socket,
    machine: Machine,
    initial: Option<Value>,
    prompt: String,
    model: String,
    public_id: String,
    deadline: Instant,
    heartbeat: Instant,
}
impl Turn {
    async fn open(
        client: &rquest::Client,
        timeout: Duration,
        payload: &ProviderAccountPayload,
        request: &CanonicalRelayRequest,
        model: &str,
    ) -> Result<Self, GatewayError> {
        let prompt = request::prompt(request, model)?.to_owned();
        let headers = session::headers(payload)?;
        if timeout.is_zero() {
            return Err(dial::timeout());
        }
        let deadline = Instant::now() + timeout.min(Duration::from_secs(120));
        let socket = tokio::time::timeout_at(deadline, dial::open(client, headers))
            .await
            .map_err(|_| dial::timeout())??;
        let initial_id = format!("evt_init_{}", uuid::Uuid::new_v4());
        Ok(Self {
            socket,
            machine: Machine::new(initial_id.clone()),
            initial: Some(request::initial(request::mode(model).unwrap(), &initial_id)),
            prompt,
            model: model.into(),
            public_id: format!("chatcmpl-{}", uuid::Uuid::new_v4()),
            deadline,
            heartbeat: Instant::now() + Duration::from_secs(25),
        })
    }
    async fn write(&mut self, value: Value) -> Result<(), GatewayError> {
        tokio::time::timeout_at(
            self.deadline.min(Instant::now() + Duration::from_secs(15)),
            self.socket.send(Message::Text(value.to_string())),
        )
        .await
        .map_err(|_| dial::timeout())?
        .map_err(|_| failure("grok_web_write_failed", ErrorKind::Network))
    }
    async fn next(&mut self) -> Result<Option<String>, GatewayError> {
        let deadline = self.deadline;
        tokio::time::timeout_at(deadline, self.read())
            .await
            .map_err(|_| dial::timeout())?
    }
    async fn read(&mut self) -> Result<Option<String>, GatewayError> {
        if let Some(initial) = self.initial.take() {
            self.write(initial).await?;
        }
        loop {
            let frame = tokio::select! {
                frame = self.socket.next() => frame,
                _ = tokio::time::sleep_until(self.heartbeat) => {
                    self.write(json!({"event":{"type":"ping","event_id":format!("evt_hb_{}",uuid::Uuid::new_v4())}})).await?;
                    self.heartbeat = Instant::now() + Duration::from_secs(25);
                    continue;
                }
            };
            match frame {
                Some(Ok(Message::Text(raw))) => match self.machine.push(&raw)? {
                    Action::None => {}
                    Action::Turn(session) => {
                        let events = request::turn(&session, &self.prompt);
                        self.prompt.clear();
                        for event in events {
                            self.write(event).await?;
                        }
                    }
                    Action::Text(text) => return Ok(Some(text)),
                    Action::Done => return Ok(None),
                },
                Some(Ok(Message::Ping(_))) => {
                    // tungstenite queued the RFC pong; flush on the same writer/deadline.
                    tokio::time::timeout_at(
                        self.deadline.min(Instant::now() + Duration::from_secs(15)),
                        self.socket.flush(),
                    )
                    .await
                    .map_err(|_| dial::timeout())?
                    .map_err(|_| failure("grok_web_write_failed", ErrorKind::Network))?;
                }
                Some(Ok(Message::Pong(_))) => {}
                Some(Ok(Message::Binary(_))) => return Err(super::machine::protocol()),
                _ => return Err(failure("grok_web_incomplete", ErrorKind::ServerError)),
            }
        }
    }
    fn chunk(&self, text: Option<String>) -> Bytes {
        let choice = match text {
            Some(text) => json!({"index":0,"delta":{"content":text},"finish_reason":null}),
            None => json!({"index":0,"delta":{},"finish_reason":"stop"}),
        };
        Bytes::from(format!(
            "data: {}\n\n{}",
            json!({
                "id":self.public_id,"object":"chat.completion.chunk","model":self.model,"choices":[choice]
            }),
            if choice["finish_reason"] == "stop" {
                "data: [DONE]\n\n"
            } else {
                ""
            }
        ))
    }
}

pub(crate) async fn execute(
    client: &rquest::Client,
    timeout: Duration,
    payload: &ProviderAccountPayload,
    request: &CanonicalRelayRequest,
    model: &str,
) -> Result<CanonicalRelayResponse, GatewayError> {
    let mut turn = Turn::open(client, timeout, payload, request, model).await?;
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
        usage: None,
        tool_calls: Vec::new(),
        upstream_status: Some(101),
        finish_reason: Some("stop".into()),
    })
}

pub(crate) async fn execute_stream(
    client: &rquest::Client,
    timeout: Duration,
    payload: &ProviderAccountPayload,
    request: &CanonicalRelayRequest,
    model: &str,
) -> Result<UpstreamStreamingResponse, GatewayError> {
    let turn = Turn::open(client, timeout, payload, request, model).await?;
    let stream = futures::stream::unfold(Some(turn), |turn| async move {
        let mut turn = turn?;
        match turn.next().await {
            Ok(Some(text)) => Some((Ok(turn.chunk(Some(text))), Some(turn))),
            Ok(None) => Some((Ok(turn.chunk(None)), None)),
            Err(error) => Some((
                Err(StreamError::<rquest::Error>::Protocol(
                    ProtocolStreamError::invalid_data(
                        error.code.as_deref().unwrap_or("grok_web_stream_failed"),
                    ),
                )),
                None,
            )),
        }
    });
    Ok(UpstreamStreamingResponse::NativeEvents(Box::pin(stream)))
}
