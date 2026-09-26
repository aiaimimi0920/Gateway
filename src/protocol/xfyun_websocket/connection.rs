use super::frames::parse_ws_message;
use super::request::{pack_request, supports_endpoint};
use super::signing::build_signed_websocket_url;
use super::XfyunWsSocket;
use crate::error::GatewayError;
use crate::protocol::canonical::{CanonicalRelayRequest, CanonicalRelayResponse, TokenUsage};
use crate::routing::candidate::ProviderAccountPayload;
use futures::{SinkExt, StreamExt};
use rustls::crypto::ring;
use rustls::{ClientConfig, RootCertStore};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::handshake::client::Response as WsResponse;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{connect_async, connect_async_tls_with_config, Connector};
use tracing::{debug, warn};

pub async fn execute_nonstream(
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
) -> Result<CanonicalRelayResponse, GatewayError> {
    let mut socket = connect_and_send(payload, req, model).await?;
    let mut text = String::new();
    let mut usage: Option<TokenUsage> = None;
    let mut saw_any_frame = false;
    let mut finish_reason = None;

    while let Some(message) = socket.next().await {
        let frame = parse_ws_message(message, model)?;
        if !frame.content_fragments.is_empty() || frame.done || frame.usage.is_some() {
            saw_any_frame = true;
        }
        for fragment in frame.content_fragments {
            text.push_str(&fragment);
        }
        if frame.usage.is_some() {
            usage = frame.usage;
        }
        if frame.done {
            finish_reason = Some("stop".to_string());
            break;
        }
    }

    if !saw_any_frame {
        return Err(GatewayError::service_unavailable(
            "XFYun native WebSocket upstream closed before returning any response frame",
        )
        .with_code("xfyun_websocket_empty_response")
        .with_provider("xfyun_websocket_compatible"));
    }

    Ok(CanonicalRelayResponse {
        model: model.to_string(),
        text,
        usage,
        tool_calls: Vec::new(),
        upstream_status: Some(200),
        finish_reason,
    })
}

pub(super) async fn connect_and_send(
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
) -> Result<XfyunWsSocket, GatewayError> {
    if !supports_endpoint(req.endpoint_kind) {
        return Err(
            GatewayError::bad_request(
                "XFYun native WebSocket adapters support only chat/messages/completions/responses style requests",
            )
            .with_code("unsupported_xfyun_websocket_endpoint")
            .with_provider("xfyun_websocket_compatible"),
        );
    }

    let signed_url = build_signed_websocket_url(payload)?;
    let request_body = pack_request(payload, req, model)?;
    let request_text = serde_json::to_string(&request_body).map_err(|error| {
        GatewayError::server_error(format!(
            "failed to serialize XFYun WebSocket request: {error}"
        ))
        .with_code("xfyun_websocket_request_serialize_failed")
        .with_provider("xfyun_websocket_compatible")
    })?;
    let (mut socket, _) = connect_xfyun_socket(&signed_url).await.map_err(|error| {
        GatewayError::service_unavailable(format!(
            "failed to connect to XFYun native WebSocket upstream: {error}"
        ))
        .with_code("xfyun_websocket_connect_failed")
        .with_provider("xfyun_websocket_compatible")
    })?;
    socket
        .send(Message::Text(request_text.into()))
        .await
        .map_err(|error| {
            GatewayError::service_unavailable(format!(
                "failed to send XFYun native WebSocket request: {error}"
            ))
            .with_code("xfyun_websocket_send_failed")
            .with_provider("xfyun_websocket_compatible")
        })?;
    Ok(socket)
}

async fn connect_xfyun_socket(
    url: &str,
) -> Result<(XfyunWsSocket, WsResponse), tokio_tungstenite::tungstenite::Error> {
    if url.starts_with("wss://") {
        let request = url.into_client_request()?;
        let connector = Connector::Rustls(std::sync::Arc::new(build_rustls_client_config()));
        connect_async_tls_with_config(request, None, false, Some(connector)).await
    } else {
        connect_async(url).await
    }
}

fn build_rustls_client_config() -> ClientConfig {
    let mut root_store = RootCertStore::empty();
    let rustls_native_certs::CertificateResult { certs, errors, .. } =
        rustls_native_certs::load_native_certs();
    if !errors.is_empty() {
        warn!("native root CA certificate loading errors: {errors:?}");
    }
    let total_number = certs.len();
    let (number_added, number_ignored) = root_store.add_parsable_certificates(certs);
    debug!(
        "added {number_added}/{total_number} native root certificates for XFYun WebSocket connector (ignored {number_ignored})"
    );
    ClientConfig::builder_with_provider(std::sync::Arc::new(ring::default_provider()))
        .with_safe_default_protocol_versions()
        .expect("ring provider should expose a usable TLS version set")
        .with_root_certificates(root_store)
        .with_no_client_auth()
}
