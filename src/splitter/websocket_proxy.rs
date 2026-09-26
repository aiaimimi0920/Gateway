use super::http_proxy::is_hop_by_hop_header;
use super::{ManagedWorker, SplitterManager, WorkerRequestLease};
use anyhow::Context;
use axum::extract::{
    ws::{Message as AxumWsMessage, WebSocket, WebSocketUpgrade},
    OriginalUri, State,
};
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode, Uri};
use axum::response::IntoResponse;
use axum::Json;
use futures::{SinkExt, StreamExt};
use std::sync::Arc;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::protocol::Message as TungsteniteMessage;
pub(super) async fn proxy_websocket_request(
    ws: WebSocketUpgrade,
    OriginalUri(uri): OriginalUri,
    State(manager): State<SplitterManager>,
    headers: HeaderMap,
) -> impl IntoResponse {
    manager.reconcile_worker_processes().await;
    let Some(worker) = manager.routing_worker() else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({
                "error": {
                    "message": "Gateway splitter has no available worker",
                    "code": "gateway_splitter_unavailable",
                }
            })),
        )
            .into_response();
    };
    let Some(lease) = worker.try_acquire_lease() else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({
                "error": {
                    "message": "Gateway splitter worker entered drain before websocket admission",
                    "code": "gateway_splitter_unavailable",
                }
            })),
        )
            .into_response();
    };

    let worker_id = worker.id.clone();
    ws.on_upgrade(move |socket| async move {
        if let Err(error) = bridge_websocket_to_worker(worker, socket, uri, headers, lease).await {
            tracing::warn!(
                worker_id = %worker_id,
                ?error,
                "Failed to proxy websocket request to active worker"
            );
        }
    })
    .into_response()
}

async fn bridge_websocket_to_worker(
    worker: Arc<ManagedWorker>,
    socket: WebSocket,
    uri: Uri,
    headers: HeaderMap,
    _lease: WorkerRequestLease,
) -> anyhow::Result<()> {
    let target = format!(
        "{}{}",
        worker_ws_base_url(&worker.base_url),
        uri.path_and_query()
            .map(|value| value.as_str())
            .unwrap_or("/")
    );

    let mut upstream_request = target
        .into_client_request()
        .context("failed to build websocket worker request")?;
    for (name, value) in &headers {
        if !is_hop_by_hop_header(name) && !is_websocket_handshake_header(name) {
            upstream_request
                .headers_mut()
                .insert(name.clone(), value.clone());
        }
    }
    upstream_request.headers_mut().insert(
        HeaderName::from_static("x-gateway-splitter-worker-id"),
        HeaderValue::from_str(worker.id.as_str())
            .unwrap_or_else(|_| HeaderValue::from_static("unknown")),
    );

    let (upstream_socket, _) = connect_async(upstream_request)
        .await
        .context("failed to open websocket connection to worker")?;
    relay_websocket(socket, upstream_socket).await
}

async fn relay_websocket(
    downstream: WebSocket,
    upstream: tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
) -> anyhow::Result<()> {
    let (mut downstream_tx, mut downstream_rx) = downstream.split();
    let (mut upstream_tx, mut upstream_rx) = upstream.split();

    let downstream_to_upstream = async {
        while let Some(message) = downstream_rx.next().await {
            let message = message.context("failed to receive downstream websocket frame")?;
            upstream_tx
                .send(axum_message_to_tungstenite(message))
                .await
                .context("failed to forward downstream websocket frame")?;
        }
        upstream_tx
            .close()
            .await
            .context("failed to close upstream websocket writer")
    };

    let upstream_to_downstream = async {
        while let Some(message) = upstream_rx.next().await {
            let message = message.context("failed to receive upstream websocket frame")?;
            if let Some(message) = tungstenite_message_to_axum(message) {
                downstream_tx
                    .send(message)
                    .await
                    .context("failed to forward upstream websocket frame")?;
            }
        }
        downstream_tx
            .close()
            .await
            .context("failed to close downstream websocket writer")
    };

    let _ = tokio::try_join!(downstream_to_upstream, upstream_to_downstream)?;
    Ok(())
}

fn axum_message_to_tungstenite(message: AxumWsMessage) -> TungsteniteMessage {
    match message {
        AxumWsMessage::Text(text) => TungsteniteMessage::Text(text.to_string().into()),
        AxumWsMessage::Binary(bytes) => TungsteniteMessage::Binary(bytes),
        AxumWsMessage::Ping(bytes) => TungsteniteMessage::Ping(bytes),
        AxumWsMessage::Pong(bytes) => TungsteniteMessage::Pong(bytes),
        AxumWsMessage::Close(frame) => TungsteniteMessage::Close(frame.map(|frame| {
            tokio_tungstenite::tungstenite::protocol::CloseFrame {
                code: frame.code.into(),
                reason: frame.reason.to_string().into(),
            }
        })),
    }
}

fn tungstenite_message_to_axum(message: TungsteniteMessage) -> Option<AxumWsMessage> {
    match message {
        TungsteniteMessage::Text(text) => Some(AxumWsMessage::Text(text)),
        TungsteniteMessage::Binary(bytes) => Some(AxumWsMessage::Binary(bytes)),
        TungsteniteMessage::Ping(bytes) => Some(AxumWsMessage::Ping(bytes)),
        TungsteniteMessage::Pong(bytes) => Some(AxumWsMessage::Pong(bytes)),
        TungsteniteMessage::Close(frame) => Some(AxumWsMessage::Close(frame.map(|frame| {
            axum::extract::ws::CloseFrame {
                code: frame.code.into(),
                reason: frame.reason.to_string().into(),
            }
        }))),
        TungsteniteMessage::Frame(_) => None,
    }
}

fn worker_ws_base_url(base_url: &str) -> String {
    if let Some(rest) = base_url.strip_prefix("https://") {
        return format!("wss://{rest}");
    }
    if let Some(rest) = base_url.strip_prefix("http://") {
        return format!("ws://{rest}");
    }
    base_url.to_string()
}

fn is_websocket_handshake_header(name: &HeaderName) -> bool {
    let lower = name.as_str().to_ascii_lowercase();
    lower == "sec-websocket-key"
        || lower == "sec-websocket-version"
        || lower == "sec-websocket-extensions"
        || lower == "sec-websocket-protocol"
        || lower == "sec-websocket-accept"
}
