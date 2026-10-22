//! One upstream socket per downstream connection; each create still runs the pipeline.
use super::*;
use futures::{SinkExt, StreamExt};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use tokio_tungstenite_native::tungstenite::Message;

type Socket = dial::Socket;
struct Connection {
    binding: [u8; 32],
    socket: Socket,
}

/// No background relay or unbounded queue. A cancelled generation owns and drops
/// its socket; only a terminal response returns it to this connection-local slot.
#[derive(Clone, Default)]
pub(crate) struct NativeResponsesSocket(
    Arc<tokio::sync::Mutex<Option<Connection>>>,
    Arc<std::sync::OnceLock<reqwest::Client>>,
);

impl NativeResponsesSocket {
    pub(super) fn client(&self, timeout: std::time::Duration) -> &reqwest::Client {
        self.1
            .get_or_init(|| crate::http_client::native_codex_websocket_client(timeout))
    }
}

pub(super) async fn send(
    client: &UpstreamClient,
    session: &NativeResponsesSocket,
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
    extra: Option<&HashMap<String, String>>,
    wire: &NativeRequestWire,
) -> Result<UpstreamStreamingResponse, GatewayError> {
    let mut plan = UpstreamClient::build_request_plan(payload, req, model, true)?;
    let mut body = plan
        .body
        .take()
        .ok_or_else(|| GatewayError::bad_request("Missing WS body"))?;
    let map = body
        .as_object_mut()
        .ok_or_else(|| GatewayError::bad_request("Invalid WS body"))?;
    // Codex 0.155.1 sends stream:true even on WS; newer SDKs omit it.
    // Preserve that actual caller shape instead of enforcing documentation drift.
    if req.raw_body.get("stream").is_none() {
        map.remove("stream");
    }
    map.insert("type".into(), Value::String("response.create".into()));
    let text = match wire.unchanged_body(&body) {
        Some(raw) => String::from_utf8(raw.to_vec())
            .map_err(|_| GatewayError::bad_request("Invalid WS UTF-8"))?,
        None => body.to_string(),
    };
    plan.method = Method::GET;
    let builder = client.native_request(payload, req, model, extra, Some(wire), &plan)?;
    let request = builder
        .try_clone()
        .ok_or_else(|| GatewayError::server_error("Cannot prepare WS handshake"))?
        .build()
        .map_err(|_| GatewayError::bad_request("Invalid WS handshake"))?;
    // Bind to target and selected credentials, not per-turn metadata. Never log
    // or expose this digest; credential changes cannot reuse an authenticated socket.
    let mut identity = Sha256::new();
    identity.update(request.url().as_str());
    for name in ["authorization", "api-key", "x-api-key"] {
        if let Some(value) = request.headers().get(name) {
            identity.update(value.as_bytes());
        }
    }
    identity.update(payload.credential_id.as_deref().unwrap_or(""));
    // Provider-owned headers may carry tenant/account auth beyond Bearer.
    let mut configured: Vec<_> = payload.headers.iter().collect();
    configured.sort_by(|a, b| a.0.cmp(b.0));
    for (name, value) in configured {
        identity.update(name.len().to_le_bytes());
        identity.update(name);
        identity.update(value.len().to_le_bytes());
        identity.update(value);
    }
    let binding: [u8; 32] = identity.finalize().into();
    let mut slot = session.0.clone().lock_owned().await;
    if slot
        .as_ref()
        .is_some_and(|connection| connection.binding != binding)
    {
        return Err(GatewayError::bad_request(
            "WebSocket route or credentials changed; reconnect before continuing",
        ));
    }
    let connection = match slot.take() {
        Some(connection) => connection,
        None => Connection {
            binding,
            socket: connect(builder, &payload.adapter).await?,
        },
    };
    let stream = events::stream(connection, slot, text);
    Ok(UpstreamStreamingResponse::NativeEvents(Box::pin(stream)))
}

#[path = "native_websocket_events.rs"]
mod events;

#[path = "native_websocket_handshake.rs"]
mod handshake;

#[path = "native_websocket_deflate.rs"]
mod deflate;

#[cfg(test)]
#[path = "native_websocket_transport_tests.rs"]
mod transport_tests;

fn protocol_error(message: &str) -> crate::protocol::stream_error::StreamError<rquest::Error> {
    crate::protocol::stream_error::ProtocolStreamError::invalid_data(message).into()
}

use dial::connect;

#[path = "native_websocket_dial.rs"]
mod dial;
